//! 联系人 CSV 方言读取/写出（自研轻量实现，不引第三方 crate）。
//!
//! 读取侧覆盖三种方言（表头嗅探，大小写与内部空白不敏感）：
//!
//! - **Gmail**：`First Name` / `Last Name` 拆列 + `E-mail 1 - Value`、
//!   `E-mail 2 - Value` 等多个邮箱列（首列为联系人主邮箱，其余并入额外邮箱）
//! - **Outlook**：`姓` / `名`（中文拼接，姓在前）或 `First Name` / `Last Name`
//!   + `E-mail Address` / `电子邮件地址`
//! - **通用**：`name` / `email` 单列姓名（本应用导出格式为其超集：
//!   `name,email,extra_emails,phone,phones_extra,org,notes,groups,starred,frequency`）
//!
//! 嗅探顺序：先按表头名定位邮箱列（找不到时回退到首个数据行中含 `@`
//! 的列），再看姓名由一列还是两列拼接。
//!
//! 手写 CSV 读取：处理双引号包裹、`""` 转义、字段内逗号/换行（含 CRLF）；
//! 写出固定带 UTF-8 BOM、CRLF 行结束，值含逗号/引号/换行时按 RFC 4180 转义。

use crate::error::MailError;
use crate::infrastructure::storage::repository::contact_repo::ContactWrite;

/// 本应用 CSV 导出表头（固定顺序，见 task spec §3.4）
pub const EXPORT_COLUMNS: &[&str] = &[
    "name",
    "email",
    "extra_emails",
    "phone",
    "phones_extra",
    "org",
    "notes",
    "groups",
    "starred",
    "frequency",
];

/// CSV 表头嗅探出的方言（仅用于识别说明，行转换行为由列映射决定）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// Gmail 导出：英文拆列姓名 + `E-mail <n> - Value` 多邮箱列
    Gmail,
    /// Outlook 导出：中文 姓/名 拼接或 `E-mail Address` 邮箱列
    Outlook,
    /// 通用方言：单列 name + email（本应用导出格式的超集）
    Generic,
}

/// 一行联系人数据（写入结构 + 目标分组名）
#[derive(Debug, Clone, Default)]
pub struct CsvContactRow {
    pub write: ContactWrite,
    pub groups: Vec<String>,
}

/// CSV 解析结果：可导入行 + 被跳过行的计数与原因
#[derive(Debug, Clone, Default)]
pub struct CsvImportOutcome {
    pub rows: Vec<CsvContactRow>,
    pub skipped: usize,
    pub errors: Vec<String>,
}

/// 表头单元格规范化：去首尾空白、内部连续空白折叠为单空格、转小写
fn norm_header(cell: &str) -> String {
    cell.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// 表头 → 列下标映射（方言无关的统一映射）
struct ColumnMap {
    /// 邮箱列（Gmail 多列按出现顺序：首个非空为主邮箱，其余为额外邮箱）
    email_cols: Vec<usize>,
    /// 名（First Name / 名）
    first_col: Option<usize>,
    /// 姓（Last Name / 姓）
    last_col: Option<usize>,
    /// 单列姓名（name / 姓名）
    single_name_col: Option<usize>,
    /// 姓名为中文 姓/名 风格（拼接时姓在前、无空格）
    chinese_style_name: bool,
    /// 电话列（可多列）
    phone_cols: Vec<usize>,
    /// 额外邮箱单列（本应用导出格式，分号分隔多值）
    extra_emails_col: Option<usize>,
    /// 额外电话单列（本应用导出格式，分号分隔多值）
    phones_extra_col: Option<usize>,
    org_col: Option<usize>,
    notes_col: Option<usize>,
    /// 分组列（可多列，单元格内分号分隔多值）
    group_cols: Vec<usize>,
}

impl ColumnMap {
    fn new() -> Self {
        Self {
            email_cols: Vec::new(),
            first_col: None,
            last_col: None,
            single_name_col: None,
            chinese_style_name: false,
            phone_cols: Vec::new(),
            extra_emails_col: None,
            phones_extra_col: None,
            org_col: None,
            notes_col: None,
            group_cols: Vec::new(),
        }
    }
}

/// 逐列分类表头（先精确匹配本应用专用列名，再按前缀/别名归类）
fn classify(header: &[String]) -> ColumnMap {
    let mut map = ColumnMap::new();
    for (index, cell) in header.iter().enumerate() {
        let h = norm_header(cell);
        // 本应用导出专用列须先于通用前缀匹配（如 "phones_extra" 以 "phone" 开头）
        match h.as_str() {
            "extra_emails" | "extra emails" | "extra email" => {
                map.extra_emails_col = Some(index);
                continue;
            }
            "phones_extra" | "phones extra" => {
                map.phones_extra_col = Some(index);
                continue;
            }
            _ => {}
        }
        // 邮箱列：E-mail …（Gmail - Value / Outlook Address）、email、中文别名
        if h.starts_with("e-mail")
            || h.starts_with("email")
            || h.starts_with("电子邮件")
            || h == "邮箱"
            || h == "邮件"
            || h == "邮件地址"
        {
            map.email_cols.push(index);
            continue;
        }
        if h.starts_with("phone") {
            map.phone_cols.push(index);
            continue;
        }
        match h.as_str() {
            "first name" | "given name" => {
                if map.first_col.is_none() {
                    map.first_col = Some(index);
                }
            }
            "last name" | "surname" | "family name" => {
                if map.last_col.is_none() {
                    map.last_col = Some(index);
                }
            }
            "名" | "名字" => {
                if map.first_col.is_none() {
                    map.first_col = Some(index);
                    map.chinese_style_name = true;
                }
            }
            "姓" | "姓氏" => {
                if map.last_col.is_none() {
                    map.last_col = Some(index);
                    map.chinese_style_name = true;
                }
            }
            "name" | "姓名" | "全名" | "display name" | "显示名称" => {
                if map.single_name_col.is_none() {
                    map.single_name_col = Some(index);
                }
            }
            "telephone" | "tel" | "电话" | "电话号码" | "手机" | "手机号码" | "移动电话" => {
                map.phone_cols.push(index);
            }
            "org" | "organization" | "organisation" | "company" | "公司" | "组织" | "单位"
            | "公司名称" | "工作单位" => {
                if map.org_col.is_none() {
                    map.org_col = Some(index);
                }
            }
            "notes" | "note" | "备注" | "注释" | "备注信息" | "remark" | "remarks" => {
                if map.notes_col.is_none() {
                    map.notes_col = Some(index);
                }
            }
            "group" | "groups" | "分组" | "分组名称" => map.group_cols.push(index),
            _ => {}
        }
    }
    map
}

/// 嗅探 CSV 表头方言
pub fn detect_dialect(header: &[String]) -> Dialect {
    let map = classify(header);
    if map.first_col.is_some() && map.last_col.is_some() {
        if map.chinese_style_name {
            return Dialect::Outlook;
        }
        let outlook_email = map.email_cols.iter().any(|&col| {
            let h = norm_header(&header[col]);
            h == "e-mail address" || h == "email address" || h.starts_with("电子邮件")
        });
        if outlook_email {
            Dialect::Outlook
        } else {
            Dialect::Gmail
        }
    } else {
        Dialect::Generic
    }
}

/// 读取单元格（越界返回空串），去首尾空白
fn cell(row: &[String], col: usize) -> String {
    row.get(col)
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

/// 可选单元格：空值（含越界）返回 `None`
fn optional_cell(row: &[String], col: Option<usize>) -> Option<String> {
    col.map(|col| cell(row, col))
        .filter(|value| !value.is_empty())
}

/// 分号分隔的多值单元格拆分（去空白、丢弃空段）
fn split_multi(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// 拼接姓名：两列（中文 姓+名 无空格 / 英文 First + Last 空格）或单列
fn compose_name(row: &[String], map: &ColumnMap) -> Option<String> {
    if let (Some(first_col), Some(last_col)) = (map.first_col, map.last_col) {
        let first = cell(row, first_col);
        let last = cell(row, last_col);
        let joined = if map.chinese_style_name {
            format!("{last}{first}")
        } else {
            format!("{first} {last}")
        };
        let joined = joined.trim().to_string();
        return (!joined.is_empty()).then_some(joined);
    }
    optional_cell(row, map.single_name_col)
}

/// 单个数据行 → 联系人行；缺邮箱计入 `skipped` / `errors`
fn convert_row(
    row: &[String],
    map: &ColumnMap,
    line_no: usize,
    outcome: &mut CsvImportOutcome,
) {
    // 邮箱列按顺序取值：首个非空为主邮箱，其余为额外邮箱
    let mut email = String::new();
    let mut extras_from_cols: Vec<String> = Vec::new();
    for col in &map.email_cols {
        let value = cell(row, *col);
        if value.is_empty() {
            continue;
        }
        if email.is_empty() {
            email = value;
        } else {
            extras_from_cols.push(value);
        }
    }
    if email.is_empty() {
        outcome.skipped += 1;
        outcome.errors.push(format!("CSV 第 {line_no} 行缺少邮箱，已跳过"));
        return;
    }

    // 额外邮箱：邮箱列余量 + extra_emails 列，忽略大小写去重并排除主邮箱
    let email_key = email.to_lowercase();
    let mut extra_emails: Vec<String> = Vec::new();
    let push_extra = |list: &mut Vec<String>, value: String| {
        let key = value.to_lowercase();
        if key != email_key && !list.iter().any(|e| e.to_lowercase() == key) {
            list.push(value);
        }
    };
    for value in extras_from_cols {
        push_extra(&mut extra_emails, value);
    }
    for value in split_multi(&cell_opt(row, map.extra_emails_col)) {
        push_extra(&mut extra_emails, value);
    }

    // 电话：电话列 + phones_extra 列，去重保序
    let mut phones: Vec<String> = map.phone_cols.iter().map(|col| cell(row, *col)).collect();
    phones.retain(|value| !value.is_empty());
    for value in split_multi(&cell_opt(row, map.phones_extra_col)) {
        if !phones.contains(&value) {
            phones.push(value);
        }
    }

    // 分组：分组列逐列逐段收集，去重保序
    let mut groups: Vec<String> = Vec::new();
    for col in &map.group_cols {
        for part in split_multi(&cell(row, *col)) {
            if !groups.contains(&part) {
                groups.push(part);
            }
        }
    }

    outcome.rows.push(CsvContactRow {
        write: ContactWrite {
            email,
            extra_emails: Some(extra_emails),
            name: compose_name(row, map),
            phones: Some(phones),
            notes: optional_cell(row, map.notes_col),
            org: optional_cell(row, map.org_col),
        },
        groups,
    });
}

/// 可选列读取（`None` 列返回空串，供多值拆分使用）
fn cell_opt(row: &[String], col: Option<usize>) -> String {
    col.map(|col| cell(row, col)).unwrap_or_default()
}

/// 手写 CSV 记录读取：双引号包裹、`""` 转义、字段内逗号/换行、CRLF/LF/CR 行结束
///
/// 开头 UTF-8 BOM 被剥掉；完全空白的行被丢弃。
fn parse_records(text: &str) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => record.push(std::mem::take(&mut field)),
                '\r' | '\n' => {
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    record.push(std::mem::take(&mut field));
                    records.push(std::mem::take(&mut record));
                }
                _ => field.push(c),
            }
        }
    }
    // 文件不以行结束符结尾时补收最后一条记录
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }

    // 丢弃空白行（仅一个空字段的记录）
    records.retain(|record| !(record.len() == 1 && record[0].trim().is_empty()));
    records
}

/// 解析 CSV 文本为联系人行集合（含方言嗅探与逐行转换）
///
/// 无表头、或无法定位邮箱列（表头不识别且首个数据行无 `@`）时返回
/// [`MailError::InvalidParam`]；数据行缺邮箱计入 `skipped` / `errors`。
pub fn parse_contacts(text: &str) -> Result<CsvImportOutcome, MailError> {
    let records = parse_records(text);
    let Some(header) = records.first() else {
        return Err(MailError::InvalidParam("CSV 内容为空或缺少表头".to_string()));
    };

    let mut map = classify(header);
    if map.email_cols.is_empty() {
        // 回退嗅探：首个数据行中含 '@' 的列视为邮箱列
        match records.get(1).and_then(|row| row.iter().position(|c| c.contains('@'))) {
            Some(col) => map.email_cols.push(col),
            None => {
                return Err(MailError::InvalidParam(
                    "无法识别 CSV 表头：未找到邮箱列".to_string(),
                ))
            }
        }
    }

    let mut outcome = CsvImportOutcome::default();
    for (index, row) in records.iter().enumerate().skip(1) {
        convert_row(row, &map, index + 1, &mut outcome);
    }
    Ok(outcome)
}

/// CSV 字段转义：含逗号/引号/换行时整体加引号并把内部 `"` 写成 `""`
pub fn escape_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// 渲染完整 CSV 文本：UTF-8 BOM + 逐行转义 + CRLF 行结束
pub fn render_csv(rows: &[Vec<String>]) -> String {
    let mut out = String::from("\u{feff}");
    for row in rows {
        let line = row
            .iter()
            .map(|field| escape_field(field))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|s| s.to_string()).collect()
    }

    // ── 方言嗅探 ────────────────────────────────────────────────

    #[test]
    fn detect_dialect_gmail_outlook_en_zh_generic() {
        // Gmail：拆列姓名 + "E-mail 1 - Value" 风格邮箱列
        assert_eq!(
            detect_dialect(&h(&["First Name", "Last Name", "E-mail 1 - Value", "E-mail 2 - Value"])),
            Dialect::Gmail
        );
        // Outlook 英文：E-mail Address
        assert_eq!(
            detect_dialect(&h(&["First Name", "Last Name", "E-mail Address"])),
            Dialect::Outlook
        );
        // Outlook 中文：姓/名 + 电子邮件地址
        assert_eq!(
            detect_dialect(&h(&["姓", "名", "电子邮件地址", "公司"])),
            Dialect::Outlook
        );
        // 通用：name,email
        assert_eq!(detect_dialect(&h(&["name", "email"])), Dialect::Generic);
        // 本应用导出表头 → 通用方言超集
        assert_eq!(
            detect_dialect(&h(EXPORT_COLUMNS)),
            Dialect::Generic,
            "导出表头应嗅探为通用方言"
        );
    }

    // ── Gmail：拆列姓名 + 多邮箱列 ──────────────────────────────

    #[test]
    fn parse_gmail_joins_two_name_columns_and_extra_email_columns() {
        let text = concat!(
            "First Name,Last Name,E-mail 1 - Value,E-mail 2 - Value\r\n",
            "John,Doe,john@x.com,john2@x.com\r\n",
            "Jane,,jane@x.com,\r\n",
        );
        let outcome = parse_contacts(text).unwrap();
        assert_eq!(outcome.rows.len(), 2);
        assert_eq!(outcome.skipped, 0);

        let john = &outcome.rows[0];
        assert_eq!(john.write.email, "john@x.com");
        assert_eq!(john.write.name.as_deref(), Some("John Doe"), "First + Last 以空格拼接");
        assert_eq!(john.write.extra_emails, Some(vec!["john2@x.com".to_string()]), "第二邮箱列并入 extra_emails");
        assert!(john.groups.is_empty());

        let jane = &outcome.rows[1];
        assert_eq!(jane.write.email, "jane@x.com");
        assert_eq!(jane.write.name.as_deref(), Some("Jane"), "Last 为空时姓名只剩 First");
        assert_eq!(jane.write.extra_emails, Some(Vec::new()));
    }

    // ── Outlook 中文：姓/名 拼接（姓在前，无空格） ───────────────

    #[test]
    fn parse_outlook_chinese_concat_surname_first() {
        let text = concat!(
            "姓,名,电子邮件地址,公司\r\n",
            "张,三,zhang@x.com,示例公司\r\n",
        );
        let outcome = parse_contacts(text).unwrap();
        assert_eq!(outcome.rows.len(), 1);
        let row = &outcome.rows[0];
        assert_eq!(row.write.email, "zhang@x.com");
        assert_eq!(row.write.name.as_deref(), Some("张三"), "中文姓名 = 姓+名 无空格拼接");
        assert_eq!(row.write.org.as_deref(), Some("示例公司"));
    }

    // ── 通用方言 + 本应用导出列 ──────────────────────────────────

    #[test]
    fn parse_generic_phone_group_and_postium_export_columns() {
        // 纯通用方言：name,email,phone,group
        let text = concat!(
            "name,email,phone,group\r\n",
            "Bob,bob@x.com,13800000001,工作\r\n",
        );
        let outcome = parse_contacts(text).unwrap();
        assert_eq!(outcome.rows.len(), 1);
        let row = &outcome.rows[0];
        assert_eq!(row.write.name.as_deref(), Some("Bob"));
        assert_eq!(row.write.email, "bob@x.com");
        assert_eq!(row.write.phones, Some(vec!["13800000001".to_string()]));
        assert_eq!(row.groups, vec!["工作".to_string()]);

        // 本应用导出格式（通用方言超集）：分号多值 + 转义过的备注
        let text = concat!(
            "name,email,extra_emails,phone,phones_extra,org,notes,groups,starred,frequency\r\n",
            "\"Alice, the Admin\",alice@x.com,a2@x.com;a3@x.com,13800000002,13900000002;12000000002,",
            "Postium,\"备注, 含逗号与\"\"引号\"\"\",工作;朋友,true,7\r\n",
        );
        let outcome = parse_contacts(text).unwrap();
        assert_eq!(outcome.rows.len(), 1);
        let row = &outcome.rows[0];
        assert_eq!(row.write.email, "alice@x.com");
        assert_eq!(row.write.name.as_deref(), Some("Alice, the Admin"), "引号内逗号应完整保留");
        assert_eq!(
            row.write.extra_emails,
            Some(vec!["a2@x.com".to_string(), "a3@x.com".to_string()])
        );
        assert_eq!(
            row.write.phones,
            Some(vec![
                "13800000002".to_string(),
                "13900000002".to_string(),
                "12000000002".to_string()
            ])
        );
        assert_eq!(row.write.org.as_deref(), Some("Postium"));
        assert_eq!(row.write.notes.as_deref(), Some("备注, 含逗号与\"引号\""));
        assert_eq!(row.groups, vec!["工作".to_string(), "朋友".to_string()]);
    }

    // ── 引号转义 / BOM / 缺邮箱跳过 ─────────────────────────────

    #[test]
    fn parse_quoted_fields_bom_crlf_and_skip_rows_without_email() {
        let text = concat!(
            "\u{feff}\"Last Name, quoted\",email\r\n",
            "\"Doe, Jane\",jane@x.com\r\n",
            "\"He said \"\"hi\"\"\",\r\n",
        );
        let outcome = parse_contacts(text).unwrap();
        assert_eq!(outcome.rows.len(), 1, "缺邮箱行应被跳过");
        assert_eq!(outcome.skipped, 1);
        assert_eq!(outcome.errors.len(), 1);
        assert!(outcome.errors[0].contains("缺少邮箱"), "错误应说明缺邮箱: {}", outcome.errors[0]);

        // 引号内逗号与 "" 转义
        assert_eq!(outcome.rows[0].write.email, "jane@x.com");
        assert_eq!(outcome.rows[0].write.name, None, "表头未识别出姓名列时不猜名字");
    }

    #[test]
    fn parse_rejects_empty_or_unrecognizable_text() {
        assert!(parse_contacts("").is_err(), "空文本应报 InvalidParam");
        assert!(
            parse_contacts("foo,bar\r\nbaz,qux\r\n").is_err(),
            "无邮箱列且数据行无 @ 应报 InvalidParam"
        );
    }

    // ── 写出：BOM + 表头 + 转义 ─────────────────────────────────

    #[test]
    fn render_csv_bom_header_and_escaping() {
        let rows = vec![
            EXPORT_COLUMNS.iter().map(|s| s.to_string()).collect(),
            vec![
                "Doe, Jane".to_string(),
                "jane@x.com".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "line1\nline2".to_string(),
                "say \"hi\"".to_string(),
                "false".to_string(),
                "3".to_string(),
            ],
        ];
        let out = render_csv(&rows);
        assert!(out.starts_with('\u{feff}'), "应带 UTF-8 BOM");
        assert!(
            out.starts_with("\u{feff}name,email,extra_emails,phone,phones_extra,org,notes,groups,starred,frequency\r\n"),
            "BOM 后应紧跟完整表头: {out:?}"
        );
        assert!(out.contains("\"Doe, Jane\""), "含逗号的值应加引号");
        assert!(out.contains("\"line1\nline2\""), "含换行的值应加引号且换行保留");
        assert!(out.contains("\"say \"\"hi\"\"\""), "内部引号应转义为两个引号");
        assert!(out.ends_with("\r\n"));
    }
}
