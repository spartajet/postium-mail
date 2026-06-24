use async_imap::imap_proto::Address;
use chrono::Datelike;

/// 返回三个月前的日期，格式化为 IMAP 搜索所需的日期格式（如 "01-Jan-2025"）
///
/// 用于全量同步时通过 IMAP `SINCE` 命令限定查询范围。
pub fn three_months_ago_imap_format() -> String {
    let now = chrono::Utc::now();
    let three_months_ago = now - chrono::Duration::days(90);

    let date_str = format!(
        "{:02}-{}-{:04}",
        three_months_ago.day(),
        month_abbr(three_months_ago.month()),
        three_months_ago.year()
    );

    tracing::info!(
        "📅 计算三个月前的日期: 现在={}, 三个月前={}, 格式化后={}",
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        three_months_ago.format("%Y-%m-%d %H:%M:%S UTC"),
        date_str
    );

    date_str
}

/// 将月份数字转换为英文缩写
fn month_abbr(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "Jan",
    }
}

/// 格式化地址列表为字符串
///
/// 将 `Address` 列表格式化为 "name <email@host>" 或 "email@host" 格式，
/// 多个地址用逗号分隔。
pub fn format_address_list(addrs: &[Address]) -> String {
    addrs
        .iter()
        .filter_map(|addr| {
            // 提取邮箱地址部分
            // mailbox 和 host 是 Option<Cow<'_, [u8]>>
            let email = match (&addr.mailbox, &addr.host) {
                (Some(mailbox), Some(host)) => {
                    // 将 Cow<[u8]> 转换为字符串（假设是 UTF-8 或 ASCII）
                    let mailbox_str = String::from_utf8_lossy(mailbox.as_ref());
                    let host_str = String::from_utf8_lossy(host.as_ref());
                    format!("{}@{}", mailbox_str, host_str)
                }
                (Some(mailbox), None) => String::from_utf8_lossy(mailbox.as_ref()).to_string(),
                _ => return None,
            };

            // 如果有显示名称，使用 "name <email>" 格式
            if let Some(name) = &addr.name {
                // name 是 Cow<'_, [u8]>，需要先转换为字符串
                let name_str = String::from_utf8_lossy(name.as_ref());
                // 解码 RFC 2047 编码的名称
                let decode_name = match rfc2047_decoder::decode(name_str.as_bytes()) {
                    Ok(decoded) => decoded,
                    Err(_) => name_str.into_owned(),
                };
                Some(format!("{} <{}>", decode_name, email))
            } else {
                Some(email)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// 从地址字符串中提取名称
///
/// # 参数
///
/// * `address` - 地址字符串（如 "John Doe <john@example.com>" 或 "john@example.com"）
///
/// # 返回
///
/// 返回名称部分（如果有）
pub fn extract_name_from_address(address: &str) -> Option<String> {
    // 地址格式: "Name <email>" 或 "email"
    if let Some(start) = address.find('<')
        && let Some(_end) = address.find('>')
    {
        let name_part = &address[..start].trim();
        if !name_part.is_empty() {
            return Some(name_part.to_string());
        }
    }
    None
}

/// 从地址字符串中提取邮箱
///
/// # 参数
///
/// * `address` - 地址字符串（如 "John Doe <john@example.com>" 或 "john@example.com"）
///
/// # 返回
///
/// 返回邮箱部分
pub fn extract_email_from_address(address: &str) -> String {
    // 地址格式: "Name <email>" 或 "email"
    if let Some(start) = address.find('<')
        && let Some(end) = address.find('>')
    {
        return address[start + 1..end].to_string();
    }
    address.to_string()
}

/// 将地址列表序列化为 JSON 数组
///
/// # 参数
///
/// * `addresses` - 地址列表（逗号分隔）
///
/// # 返回
///
/// 返回 JSON 数组字符串，如 `["email1@example.com","email2@example.com"]`
pub fn serialize_addresses(addresses: &str) -> String {
    if addresses.is_empty() {
        return "[]".to_string();
    }

    // 分割地址并提取邮箱部分
    let emails: Vec<String> = addresses
        .split(',')
        .map(|addr| extract_email_from_address(addr.trim()))
        .map(|email| format!("\"{}\"", email))
        .collect();

    format!("[{}]", emails.join(","))
}
