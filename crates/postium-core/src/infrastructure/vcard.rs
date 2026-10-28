//! vCard 解析/生成/属性级合并（自研轻量实现）。
//!
//! 覆盖 vCard 3.0/4.0 的建模属性（N/FN/EMAIL/TEL/ORG/NOTE/CATEGORIES/UID/REV），
//! 2.1 的 QUOTED-PRINTABLE（含多行软/硬换行拼接）与 RFC 2047 编码字尽力解码；
//! 未建模属性行（PHOTO/ADR/IMPP/X-* 等）原样保留在 [`VcardData::other_lines`]，
//! 供推送合并时回写。
//!
//! 生成固定 VERSION:3.0、CRLF 行结束、75 字节折行（UTF-8 字符边界安全）。

use crate::error::MailError;
use chrono::{DateTime, NaiveDate, NaiveDateTime};

/// vCard 建模属性名（emit 顺序即输出顺序，也是 merge 的替换集合）。
const MODELED_PROPS: [&str; 9] = [
    "N", "FN", "EMAIL", "TEL", "ORG", "NOTE", "CATEGORIES", "UID", "REV",
];

/// 每物理行最大字节数（RFC 折行上限 75 octets，含续行前导空格）。
const LINE_LIMIT_BYTES: usize = 75;

/// vCard 传输数据（纯文本进出；与 ContactDto/ContactWrite 的转换在 service 层完成）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VcardData {
    /// FORMATTED NAME（FN）。
    pub fn_name: Option<String>,
    /// N 原始结构值（如 `Last;First;;;`），不拆解。
    pub n_name: Option<String>,
    /// 邮箱列表，按出现顺序（第一个为 pref/首个）。
    pub emails: Vec<String>,
    /// 电话列表。
    pub phones: Vec<String>,
    /// ORG 原始结构值（如 `Company;Unit`）。
    pub org: Option<String>,
    /// NOTE 文本。
    pub note: Option<String>,
    /// CATEGORIES 拆分后的值列表。
    pub categories: Vec<String>,
    pub uid: Option<String>,
    /// REV ↔ epoch 秒（UTC）；解析失败为 None（不报错）。
    pub rev: Option<i64>,
    /// 未建模属性行原样保留（含参数、已展折行），emit/merge 时回写。
    pub other_lines: Vec<String>,
}

/// 解析 vCard 文本（3.0/4.0；2.1 QP 尽力）。
///
/// 缺少 `BEGIN:VCARD` / `END:VCARD` 时返回 [`MailError::InvalidParam`]；
/// 单个属性值解码失败尽力回退（lossy），REV 解析失败记 `None`。
pub fn parse(text: &str) -> Result<VcardData, MailError> {
    let mut d = VcardData::default();
    let mut has_begin = false;
    let mut has_end = false;

    for line in logical_lines(text) {
        let Some((left, raw_value)) = split_property(&line) else {
            // 无 ':' 的畸形行按原样归入 other_lines，避免丢数据
            d.other_lines.push(line);
            continue;
        };
        let name = prop_name(left);
        let value = decode_value(raw_value, is_quoted_printable(left));
        match name.as_str() {
            "BEGIN" => has_begin |= value.trim().eq_ignore_ascii_case("VCARD"),
            "END" => has_end |= value.trim().eq_ignore_ascii_case("VCARD"),
            "VERSION" => {}
            "N" => set_once(&mut d.n_name, &value),
            "FN" => {
                let unescaped = unescape_text(&value);
                set_once(&mut d.fn_name, &unescaped);
            }
            "EMAIL" => push_non_empty(&mut d.emails, &value),
            "TEL" => push_non_empty(&mut d.phones, &value),
            "ORG" => set_once(&mut d.org, &value),
            "NOTE" => {
                let unescaped = unescape_text(&value);
                set_once(&mut d.note, &unescaped);
            }
            "CATEGORIES" => d.categories.extend(split_categories(&value)),
            "UID" => set_once(&mut d.uid, &value),
            "REV" => {
                if d.rev.is_none() {
                    d.rev = parse_rev(&value);
                }
            }
            _ => d.other_lines.push(line),
        }
    }

    if !has_begin {
        return Err(MailError::InvalidParam(
            "vCard 格式无效: 缺少 BEGIN:VCARD".to_string(),
        ));
    }
    if !has_end {
        return Err(MailError::InvalidParam(
            "vCard 格式无效: 缺少 END:VCARD".to_string(),
        ));
    }
    Ok(d)
}

/// 生成 vCard 文本（VERSION:3.0、CRLF、75 字节折行）。
pub fn emit(d: &VcardData) -> String {
    let mut lines: Vec<String> = vec!["BEGIN:VCARD".to_string(), "VERSION:3.0".to_string()];
    for name in MODELED_PROPS {
        lines.extend(modeled_lines_for(name, d));
    }
    for other in &d.other_lines {
        if !is_framework_line(other) {
            lines.push(other.clone());
        }
    }
    lines.push("END:VCARD".to_string());
    finish(lines)
}

/// 属性级合并：以 `raw` 为基底，建模属性替换为 `d` 的值，未建模行原样保留。
///
/// - `BEGIN`/`VERSION`/`END` 三行保留 raw 原样（首尾）；
/// - 建模属性在 raw 中首次出现的位置替换为 `d` 的值（列表属性展开为多行），
///   后续同名行丢弃；`d` 中该属性为空则整组删除；
///   `d` 有而 raw 没有的建模属性按规范顺序补在主体末尾；
/// - `d.other_lines` 中 raw 未包含的行按整行文本去重后追加。
pub fn merge_into_raw(raw: &str, d: &VcardData) -> String {
    let mut body: Vec<String> = Vec::new();
    let mut begin_line: Option<String> = None;
    let mut version_line: Option<String> = None;
    let mut end_line: Option<String> = None;
    let mut handled: Vec<&str> = Vec::new();
    let mut kept: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in logical_lines(raw) {
        let name = split_property(&line).map(|(left, _)| prop_name(left));
        match name.as_deref() {
            None => {
                kept.insert(line.clone());
                body.push(line);
            }
            Some("BEGIN") => {
                if begin_line.is_none() {
                    begin_line = Some(line);
                }
            }
            Some("END") => {
                if end_line.is_none() {
                    end_line = Some(line);
                }
            }
            Some("VERSION") => {
                if version_line.is_none() {
                    version_line = Some(line);
                }
            }
            Some(n) => {
                let Some(key) = MODELED_PROPS.iter().find(|p| **p == n) else {
                    kept.insert(line.clone());
                    body.push(line);
                    continue;
                };
                if !handled.contains(key) {
                    handled.push(key);
                    body.extend(modeled_lines_for(key, d));
                }
                // 后续同名行丢弃（去重）
            }
        }
    }

    // d 中有而 raw 中没有的建模属性：按规范顺序补在主体末尾
    for key in MODELED_PROPS {
        if !handled.contains(&key) {
            body.extend(modeled_lines_for(key, d));
        }
    }
    // d.other_lines 中 raw 未包含的行补写（按整行文本去重，框架行跳过）
    for other in &d.other_lines {
        if is_framework_line(other) || kept.contains(other) {
            continue;
        }
        kept.insert(other.clone());
        body.push(other.clone());
    }

    let mut lines = Vec::with_capacity(body.len() + 3);
    lines.push(begin_line.unwrap_or_else(|| "BEGIN:VCARD".to_string()));
    lines.push(version_line.unwrap_or_else(|| "VERSION:3.0".to_string()));
    lines.extend(body);
    lines.push(end_line.unwrap_or_else(|| "END:VCARD".to_string()));
    finish(lines)
}

// ========== 解析辅助 ==========

/// 拆逻辑行（vCard 折行 + 2.1 多行 QP 值拼接），自上而下逐物理行归并：
///
/// 1. vCard 折行：续行以空格/制表符开头 → 剥掉一个前导空白后直接拼接；
///    被拼接的是 QP 属性且行尾为 `=` 时，先去掉该软换行标记；
/// 2. QP 软换行：上一逻辑行是 QUOTED-PRINTABLE 属性、行尾为 `=` → 去掉 `=`
///    与下一物理行直接拼接（多段重复适用）；
/// 3. QP 硬换行：上一逻辑行是 QP 属性且当前行不构成独立属性（引号外无 `:`）
///    → 以 `\n` 拼接，保留为值内换行。
fn logical_lines(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() {
            continue;
        }
        let joined = if let Some(last) = out.last_mut() {
            if line.starts_with(' ') || line.starts_with('\t') {
                // vCard 折行续行：剥掉一个前导空白
                if is_qp_line(last) && last.ends_with('=') {
                    last.pop(); // 物理行尾 '=' 为 QP 软换行标记
                }
                last.push_str(&line[1..]);
                true
            } else if is_qp_line(last) && !is_property_line(line) {
                if last.ends_with('=') {
                    last.pop(); // QP 软换行标记
                    last.push_str(line);
                } else {
                    last.push('\n'); // 硬换行保留为值内换行
                    last.push_str(line);
                }
                true
            } else {
                false
            }
        } else {
            false
        };
        if !joined {
            out.push(line.to_string());
        }
    }
    out
}

/// 按 `:` 拆分属性行为（名称+参数，值）；引号内的 `:` 不算分隔符。
fn split_property(line: &str) -> Option<(&str, &str)> {
    let mut quoted = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            ':' if !quoted => return Some((&line[..i], &line[i + 1..])),
            _ => {}
        }
    }
    None
}

/// 属性名：去掉参数（首个 `;` 前）、去掉 2.1 分组前缀（取 `.` 后最后一段），大写。
fn prop_name(left: &str) -> String {
    let base = left.split(';').next().unwrap_or(left);
    let base = base.rsplit('.').next().unwrap_or(base);
    base.trim().to_ascii_uppercase()
}

/// 属性是否声明了 QUOTED-PRINTABLE 编码（vCard 2.1）：参数段精确等于
/// `ENCODING=QUOTED-PRINTABLE`（或 2.1 简写 `QUOTED-PRINTABLE`）才算，
/// 属性名/其他参数值仅"包含"该子串时不算，避免普通值里的 `=XX` 被误解码。
fn is_quoted_printable(left: &str) -> bool {
    left.split(';')
        .skip(1) // 首段是属性名（可能带 2.1 分组前缀）
        .any(|p| {
            let p = p.trim().trim_matches('"').to_ascii_uppercase();
            p == "ENCODING=QUOTED-PRINTABLE" || p == "QUOTED-PRINTABLE"
        })
}

/// 逻辑行是否为 QUOTED-PRINTABLE 编码属性（多行 QP 值拼接判断用）。
fn is_qp_line(line: &str) -> bool {
    split_property(line)
        .map(|(left, _)| is_quoted_printable(left))
        .unwrap_or(false)
}

/// 物理行是否构成独立属性（引号外含 `:`）；不构成且紧随 QP 属性时视为其续段。
fn is_property_line(line: &str) -> bool {
    split_property(line).is_some()
}

/// 值解码：先按 ENCODING=QUOTED-PRINTABLE 解 =XX 十六进制，再解 RFC 2047 编码字。
fn decode_value(value: &str, qp: bool) -> String {
    let stage = if qp {
        decode_qp_hex(value)
    } else {
        value.to_string()
    };
    decode_encoded_words(&stage)
}

/// 自研 QP 解码：`=XX`（大小写十六进制）→ 字节，软换行 `=\r\n` / `=\n` 跳过，
/// 非法序列原样保留；结果按 UTF-8 lossy 转字符串。
fn decode_qp_hex(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
                continue;
            }
            if i + 2 < bytes.len() && bytes[i + 1] == b'\r' && bytes[i + 2] == b'\n' {
                i += 3;
                continue;
            }
            if let (Some(h), Some(l)) = (
                bytes.get(i + 1).copied().and_then(hex_val),
                bytes.get(i + 2).copied().and_then(hex_val),
            ) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// RFC 2047 编码字解码：`=?charset?Q?data?=` / `=?charset?B?data?=`。
/// 结构不完整或字符集不支持时原样保留，继续扫描后续 token。
fn decode_encoded_words(input: &str) -> String {
    if !input.contains("=?") {
        return input.to_string();
    }
    let mut out = String::new();
    let mut rest = input;
    while let Some(start) = rest.find("=?") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        match take_encoded_word(tail) {
            Some((charset, enc, data, consumed)) => {
                match decode_word_token(&charset, enc, data) {
                    Some(text) => {
                        out.push_str(&text);
                        rest = &tail[consumed..];
                    }
                    None => {
                        out.push_str("=?");
                        rest = tail;
                    }
                }
            }
            None => {
                out.push_str("=?");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 从 `=?` 之后截取编码字：`charset?enc?data?=`，返回（小写字符集，编码字节，
/// 数据，消耗的字节数）。
fn take_encoded_word(tail: &str) -> Option<(String, u8, &str, usize)> {
    let q1 = tail.find('?')?;
    let charset = &tail[..q1];
    let bytes = tail.as_bytes();
    if charset.is_empty() || q1 + 2 >= bytes.len() || bytes[q1 + 2] != b'?' {
        return None;
    }
    let data_start = q1 + 3;
    let end = data_start + tail[data_start..].find("?=")?;
    Some((
        charset.to_ascii_lowercase(),
        bytes[q1 + 1],
        &tail[data_start..end],
        end + 2,
    ))
}

/// 按字符集与编码（Q/B）解码编码字数据；仅支持 UTF-8/ASCII。
fn decode_word_token(charset: &str, enc: u8, data: &str) -> Option<String> {
    if !matches!(charset, "utf-8" | "utf8" | "us-ascii" | "ascii") {
        return None;
    }
    match enc {
        b'Q' | b'q' => decode_q_word(data),
        b'B' | b'b' => {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD
                .decode(data.trim())
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        }
        _ => None,
    }
}

/// Q 编码字解码：`_` → 空格，`=XX` → 字节，其余原样。
fn decode_q_word(data: &str) -> Option<String> {
    let bytes = data.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'_' => {
                out.push(b' ');
                i += 1;
            }
            b'=' => {
                let h = bytes.get(i + 1).copied().and_then(hex_val)?;
                let l = bytes.get(i + 2).copied().and_then(hex_val)?;
                out.push(h << 4 | l);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

// ========== 文本转义（FN/NOTE/CATEGORIES 值） ==========

/// vCard 文本值转义：`\` `\n` `\r` `,` `;`。
fn escape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            ',' => out.push_str("\\,"),
            ';' => out.push_str("\\;"),
            _ => out.push(c),
        }
    }
    out
}

/// [`escape_text`] 的逆操作；非法转义序列取字面字符。
fn unescape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some('r') | Some('R') => out.push('\r'),
                Some(esc) => out.push(esc),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// CATEGORIES 拆分：按未转义的 `,` 分段，每段去空白并反转义，丢弃空段。
fn split_categories(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            cur.push('\\');
            cur.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == ',' {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    parts.push(cur);
    parts
        .into_iter()
        .map(|p| unescape_text(p.trim()))
        .filter(|p| !p.is_empty())
        .collect()
}

// ========== REV ↔ epoch ==========

/// REV 字符串 → epoch 秒；支持 RFC 3339 扩展格式与 vCard 基本格式
/// （`20261028T120000Z` / `20261028T120000` / `20261028`），失败返回 None。
fn parse_rev(value: &str) -> Option<i64> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(v) {
        return Some(dt.timestamp());
    }
    let basic = v.strip_suffix('Z').or_else(|| v.strip_suffix('z')).unwrap_or(v);
    if let Ok(nd) = NaiveDateTime::parse_from_str(basic, "%Y%m%dT%H%M%S") {
        return Some(nd.and_utc().timestamp());
    }
    if let Ok(date) = NaiveDate::parse_from_str(basic, "%Y%m%d") {
        return Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp());
    }
    None
}

/// epoch 秒 → vCard 3.0 REV 基本格式（`20261028T120000Z`）；越界返回 None。
fn rev_to_iso(epoch: i64) -> Option<String> {
    DateTime::from_timestamp(epoch, 0).map(|dt| dt.format("%Y%m%dT%H%M%SZ").to_string())
}

// ========== 生成/合并辅助 ==========

/// 生成某建模属性的属性行（值取自 `d`；空值返回空组 = 该属性缺省/删除）。
fn modeled_lines_for(name: &str, d: &VcardData) -> Vec<String> {
    match name {
        "N" => d
            .n_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| vec![format!("N:{v}")])
            .unwrap_or_default(),
        "FN" => d
            .fn_name
            .as_deref()
            .map(|v| format!("FN:{}", escape_text(v)))
            .map(|l| vec![l])
            .unwrap_or_default(),
        "EMAIL" => d
            .emails
            .iter()
            .map(|e| e.trim())
            .filter(|e| !e.is_empty())
            .map(|e| format!("EMAIL;TYPE=INTERNET:{e}"))
            .collect(),
        "TEL" => d
            .phones
            .iter()
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(|p| format!("TEL:{p}"))
            .collect(),
        "ORG" => d
            .org
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| vec![format!("ORG:{v}")])
            .unwrap_or_default(),
        "NOTE" => d
            .note
            .as_deref()
            .map(|v| format!("NOTE:{}", escape_text(v)))
            .map(|l| vec![l])
            .unwrap_or_default(),
        "CATEGORIES" => {
            let cats: Vec<&str> = d
                .categories
                .iter()
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
                .collect();
            if cats.is_empty() {
                Vec::new()
            } else {
                vec![format!(
                    "CATEGORIES:{}",
                    cats.into_iter().map(escape_text).collect::<Vec<_>>().join(",")
                )]
            }
        }
        "UID" => d
            .uid
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| vec![format!("UID:{v}")])
            .unwrap_or_default(),
        "REV" => d
            .rev
            .and_then(rev_to_iso)
            .map(|v| vec![format!("REV:{v}")])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// 是否 BEGIN/VERSION/END 框架行或空行（other_lines 回写时跳过，防重复）。
fn is_framework_line(line: &str) -> bool {
    if line.trim().is_empty() {
        return true;
    }
    matches!(
        split_property(line).map(|(l, _)| prop_name(l)).as_deref(),
        Some("BEGIN") | Some("VERSION") | Some("END")
    )
}

/// 单逻辑行 75 字节折行（UTF-8 字符边界安全；续行以空格开头且计入预算）。
fn fold_line(line: &str) -> String {
    if line.len() <= LINE_LIMIT_BYTES {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + 3 * (line.len() / LINE_LIMIT_BYTES + 1));
    let mut width = 0usize;
    for c in line.chars() {
        let c_len = c.len_utf8();
        if width + c_len > LINE_LIMIT_BYTES {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += c_len;
    }
    out
}

/// 逐行折行、CRLF 连接并补结尾 CRLF；跳过空行。
fn finish(lines: Vec<String>) -> String {
    let mut out = String::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        out.push_str(&fold_line(&line));
        out.push_str("\r\n");
    }
    out
}

/// 标量字段首次出现生效（去空白，空值跳过）。
fn set_once(slot: &mut Option<String>, value: &str) {
    let value = value.trim();
    if slot.is_none() && !value.is_empty() {
        *slot = Some(value.to_string());
    }
}

/// 列表字段追加非空项（去空白）。
fn push_non_empty(list: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        list.push(value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 组 1: roundtrip ──────────────────────────────────────────────

    #[test]
    fn roundtrip_4_0_full_card() {
        let text = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:4.0\r\n",
            "N:Doe;John;;;\r\n",
            "FN:John Doe\r\n",
            "EMAIL;TYPE=INTERNET:john@example.com\r\n",
            "EMAIL;TYPE=INTERNET:john@example.org\r\n",
            "TEL;TYPE=CELL:+8613800000000\r\n",
            "ORG:Example Inc;Engineering\r\n",
            "NOTE:met at conference\r\n",
            "CATEGORIES:work,friends\r\n",
            "UID:urn:uuid:8f0a-1234\r\n",
            "REV:20261028T120000Z\r\n",
            "END:VCARD\r\n",
        );
        let d1 = parse(text).expect("parse 4.0 应成功");
        assert_eq!(d1.fn_name.as_deref(), Some("John Doe"));
        assert_eq!(d1.n_name.as_deref(), Some("Doe;John;;;"));
        assert_eq!(d1.emails, ["john@example.com", "john@example.org"]);
        assert_eq!(d1.phones, ["+8613800000000"]);
        assert_eq!(d1.org.as_deref(), Some("Example Inc;Engineering"));
        assert_eq!(d1.note.as_deref(), Some("met at conference"));
        assert_eq!(d1.categories, ["work", "friends"]);
        assert_eq!(d1.uid.as_deref(), Some("urn:uuid:8f0a-1234"));
        assert_eq!(d1.rev, parse_rev("20261028T120000Z"));
        assert!(d1.other_lines.is_empty(), "输入无未建模属性");

        let emitted = emit(&d1);
        assert!(emitted.starts_with("BEGIN:VCARD\r\nVERSION:3.0\r\n"));
        assert!(emitted.ends_with("END:VCARD\r\n"));
        assert!(emitted.contains("EMAIL;TYPE=INTERNET:john@example.com\r\n"));
        assert!(emitted.contains("CATEGORIES:work,friends\r\n"));

        let d2 = parse(&emitted).expect("re-parse emit 结果应成功");
        assert_eq!(d1, d2, "roundtrip 后 VcardData 应等价");
    }

    #[test]
    fn parse_rejects_missing_begin_or_end() {
        assert!(parse("VERSION:3.0\r\nFN:x\r\nEND:VCARD\r\n").is_err());
        assert!(parse("BEGIN:VCARD\r\nVERSION:3.0\r\nFN:x\r\n").is_err());
        assert!(parse("").is_err());
    }

    // ── 组 2: 75 字节折行/展开 ────────────────────────────────────────

    #[test]
    fn folding_emit_within_75_bytes_and_unfold() {
        // 5 字节 NOTE: + 40*18=720 字节中文 → 必然折行且必须落在字符边界
        let zh_note = "联系人备注线".repeat(40);
        let mut d = VcardData::default();
        d.fn_name = Some("张三".to_string());
        d.note = Some(zh_note.clone());
        let out = emit(&d);

        for line in out.split("\r\n") {
            assert!(
                line.len() <= LINE_LIMIT_BYTES,
                "物理行超 {LINE_LIMIT_BYTES} 字节（{} 字节）: {line:?}",
                line.len()
            );
        }
        assert!(
            out.contains("\r\n "),
            "长行应产生以空格开头的续行"
        );
        // NOTE 内容的物理行：首行以 NOTE: 开头，续行以空格开头
        let note_lines: Vec<&str> = out.split("\r\n").filter(|l| l.contains("联系人")).collect();
        assert!(note_lines.len() >= 3, "长 NOTE 应折成多个物理行");
        assert!(note_lines
            .iter()
            .all(|l| l.starts_with("NOTE:") || l.starts_with(' ')));

        let back = parse(&out).expect("折行输出应可解析");
        assert_eq!(back.note, Some(zh_note), "展开后 NOTE 应无损还原");
        assert_eq!(back.fn_name.as_deref(), Some("张三"));
    }

    #[test]
    fn parse_unfolds_crlf_space_continuation() {
        let folded = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:3.0\r\n",
            "NOTE:0123456789\r\n 0123456789\r\n",
            "END:VCARD\r\n",
        );
        let d = parse(folded).expect("折行输入应可解析");
        assert_eq!(
            d.note,
            Some("01234567890123456789".to_string()),
            "应展开 \\r\\n 空格续行"
        );
    }

    // ── 组 3: 2.1 QUOTED-PRINTABLE / RFC2047 编码字 ──────────────────

    #[test]
    fn vcard_2_1_quoted_printable_and_encoded_word() {
        let text = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:2.1\r\n",
            "N;ENCODING=QUOTED-PRINTABLE:=e6=b5=8b=e8=af=95\r\n",
            "EMAIL;ENCODING=QUOTED-PRINTABLE:=e6=b5=8b@example.com\r\n",
            "FN:=?UTF-8?Q?=e6=b5=8b=e8=af=95?=\r\n",
            "END:VCARD\r\n",
        );
        let d = parse(text).expect("parse 2.1 应成功");
        assert_eq!(d.n_name.as_deref(), Some("测试"), "N 的 =XX QP 应解出 UTF-8");
        assert_eq!(d.emails, ["测@example.com"], "EMAIL 的 =XX QP 应解出 UTF-8");
        assert_eq!(
            d.fn_name.as_deref(),
            Some("测试"),
            "=?UTF-8?Q?...?= 编码字应解出 UTF-8"
        );
    }

    #[test]
    fn vcard_2_1_qp_soft_line_breaks_join_and_decode() {
        // 两段软换行：行尾 '=' 为软换行标记，续段（无空格前缀）直接拼接；
        // FN 段额外覆盖「软换行 + vCard 空格折行续行」的组合
        let text = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:2.1\r\n",
            "N;ENCODING=QUOTED-PRINTABLE:=e6=b5=8b=\r\n",
            "=e8=af=95\r\n",
            "FN;ENCODING=QUOTED-PRINTABLE:=e6=b5=8b=\r\n",
            " =e8=af=95\r\n",
            "END:VCARD\r\n",
        );
        let d = parse(text).expect("parse 2.1 多行 QP 应成功");
        assert_eq!(d.n_name.as_deref(), Some("测试"), "软换行两段应拼接后完整解码");
        assert_eq!(
            d.fn_name.as_deref(),
            Some("测试"),
            "软换行 + 空格折行续行应拼接后完整解码"
        );
        assert!(d.other_lines.is_empty(), "QP 续段不应落入 other_lines");
    }

    #[test]
    fn vcard_2_1_qp_hard_line_break_preserved_as_newline() {
        // 首段行尾无 '='：与续段之间为硬换行，保留为值内 \n；
        // 续段之后真正的属性行不应被吞并
        let text = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:2.1\r\n",
            "NOTE;ENCODING=QUOTED-PRINTABLE:=e7=ac=ac=e4=b8=80=e8=a1=8c\r\n",
            "=e7=ac=ac=e4=ba=8c=e8=a1=8c\r\n",
            "TEL:+8613800000000\r\n",
            "END:VCARD\r\n",
        );
        let d = parse(text).expect("parse 2.1 硬换行 QP 应成功");
        assert_eq!(d.note.as_deref(), Some("第一行\n第二行"));
        assert_eq!(d.phones, ["+8613800000000"], "续段后的属性行不应被吞并");
        assert!(d.other_lines.is_empty());
    }

    #[test]
    fn qp_decoding_requires_encoding_param_exact_match() {
        // 属性名/其他参数值仅"包含" QUOTED-PRINTABLE 子串时，不得对值做 QP 解码
        let text = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:3.0\r\n",
            "EMAIL;X-SOURCE=QUOTED-PRINTABLE-STYLE:=e6=b5=8b@x.com\r\n",
            "END:VCARD\r\n",
        );
        let d = parse(text).expect("parse 应成功");
        assert_eq!(
            d.emails,
            ["=e6=b5=8b@x.com"],
            "无 ENCODING=QUOTED-PRINTABLE 参数的普通值应原样保留"
        );
    }

    // ── 组 4: merge_into_raw ─────────────────────────────────────────

    #[test]
    fn merge_preserves_unmodeled_and_replaces_modeled() {
        let raw = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:4.0\r\n",
            "N:Old;Name\r\n",
            "FN:Old Name\r\n",
            "EMAIL;TYPE=INTERNET:old@example.com\r\n",
            "EMAIL;TYPE=HOME:old2@example.com\r\n",
            "PHOTO:https://example.com/me.jpg\r\n",
            "X-CUSTOM:keep-me\r\n",
            "REV:20200101T000000Z\r\n",
            "END:VCARD\r\n",
        );
        let mut d = VcardData::default();
        d.n_name = Some("New;Name".to_string());
        d.fn_name = Some("New Name".to_string());
        d.emails = vec!["new@example.com".to_string()];
        d.uid = Some("local-1".to_string());
        d.rev = parse_rev("20261028T120000Z");

        let merged = merge_into_raw(raw, &d);

        // 头尾保留 raw 原样（VERSION 仍是 raw 的 4.0）
        assert!(merged.starts_with("BEGIN:VCARD\r\nVERSION:4.0\r\n"));
        assert!(merged.ends_with("END:VCARD\r\n"));
        // 未建模属性原样保留
        assert!(merged.contains("PHOTO:https://example.com/me.jpg"));
        assert!(merged.contains("X-CUSTOM:keep-me"));
        // 建模属性被替换
        assert!(merged.contains("N:New;Name"));
        assert!(merged.contains("FN:New Name"));
        assert!(merged.contains("EMAIL;TYPE=INTERNET:new@example.com"));
        assert!(merged.contains("UID:local-1"));
        assert!(merged.contains("REV:20261028T120000Z"));
        assert!(!merged.contains("old@example.com"), "旧 EMAIL 应被替换");
        assert!(!merged.contains("Old Name"), "旧 N/FN 值应被替换");
        assert!(!merged.contains("20200101T000000Z"), "旧 REV 应被替换");

        // 无重复属性行（续行以空格开头，不会误计）
        let line_count = |prefix: &str| {
            merged
                .lines()
                .map(|l| l.trim_start_matches('\r'))
                .filter(|l| l.starts_with(prefix))
                .count()
        };
        assert_eq!(line_count("EMAIL"), 1, "EMAIL 只剩 d 的一行");
        assert_eq!(line_count("N:"), 1);
        assert_eq!(line_count("FN:"), 1);
        assert_eq!(line_count("REV:"), 1);
        assert_eq!(line_count("PHOTO"), 1);
        assert_eq!(line_count("X-CUSTOM"), 1);

        // 合并结果可再解析，建模字段与 d 一致
        let back = parse(&merged).expect("合并结果应可解析");
        assert_eq!(back.emails, ["new@example.com"]);
        assert_eq!(back.n_name.as_deref(), Some("New;Name"));
        assert_eq!(back.fn_name.as_deref(), Some("New Name"));
        assert_eq!(back.rev, d.rev);
        assert_eq!(back.uid.as_deref(), Some("local-1"));
        assert!(back.other_lines.iter().any(|l| l.starts_with("PHOTO")));
        assert!(back.other_lines.iter().any(|l| l.starts_with("X-CUSTOM")));
    }

    #[test]
    fn merge_appends_modeled_props_absent_in_raw() {
        let raw = concat!(
            "BEGIN:VCARD\r\n",
            "VERSION:3.0\r\n",
            "FN:Only Name\r\n",
            "X-NICK:Mia\r\n",
            "END:VCARD\r\n",
        );
        let mut d = VcardData::default();
        d.fn_name = Some("Only Name".to_string());
        d.emails = vec!["a@example.com".to_string(), "b@example.com".to_string()];
        d.categories = vec!["work".to_string(), "client".to_string()];
        d.note = None; // raw 无 NOTE，d 也无 → 不新增

        let merged = merge_into_raw(raw, &d);
        assert!(merged.contains("EMAIL;TYPE=INTERNET:a@example.com"));
        assert!(merged.contains("EMAIL;TYPE=INTERNET:b@example.com"));
        assert!(merged.contains("CATEGORIES:work,client"));
        assert!(!merged.contains("NOTE:"));
        assert!(merged.contains("X-NICK:Mia"), "未建模行保留");
        let back = parse(&merged).unwrap();
        assert_eq!(back.emails.len(), 2);
        assert_eq!(back.categories, ["work", "client"]);
    }
}
