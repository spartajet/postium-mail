//! 附件视觉规则 —— 数值来源：网易邮箱大师 5.5.3.1009 webui.zip `js/common.js`
//! （`AttachUtils.colorMap` / `MailDataAccess` 大小格式化）。
//! 完整清单见仓库根 `MECHANICS.md` §3–§4 与 `mechanics.json`。

/// 文件类型分类（邮箱大师 AttachUtils.typeMap 的扩展名表，节选高频扩展名）。
pub fn file_kind(filename: &str) -> &'static str {
    let ext = filename
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "xls" | "xlsx" | "csv" | "xlsm" | "xlt" | "xltx" | "xltm" => "excel",
        "doc" | "docx" | "dot" | "dotm" | "dotx" | "docm" => "word",
        "ppt" | "pptx" | "pot" | "potm" | "potx" | "pps" | "ppsx" | "pptm" => "ppt",
        "7z" | "rar" | "cab" | "jar" | "arj" | "tar" | "gz" | "tgz" | "zip" | "war" | "ear"
        | "bz2" | "tbz" | "cpio" | "ar" | "crx" => "zip",
        "eml" => "mail",
        "htm" | "html" | "shtml" | "xhtml" | "mht" | "asp" | "aspx" => "html",
        "js" | "json" => "js",
        "css" | "ini" | "inf" | "xml" => "css",
        "ttf" | "fon" => "font",
        "pdf" => "pdf",
        "psd" | "pdd" | "ps" => "ps",
        "ai" | "eps" | "ait" => "ai",
        "fla" | "swf" | "flv" => "flash",
        "txt" => "txt",
        "ra" | "ram" | "rm" | "rmvb" | "wmv" | "asf" | "mov" | "qt" | "3gp" | "avi" | "mkv"
        | "mp4" | "mpg" | "mpeg" | "vob" | "ts" | "m2ts" => "video",
        "mpa" | "mp2" | "mp3" | "ac3" | "dts" | "flac" | "ape" | "wv" | "cda" | "wav" | "aac"
        | "m4a" | "mka" | "ogg" | "mpc" | "au" | "aif" | "aiff" | "mid" | "midi" => "music",
        "jpg" | "jpeg" | "gif" | "png" | "bmp" | "dib" | "rle" | "tif" | "tiff" | "emf"
        | "wmf" | "ico" | "cur" | "ani" | "raw" | "svg" => "img",
        "dll" | "ocx" | "chk" | "manifest" => "dll",
        "vcf" => "vcf",
        "ics" => "ics",
        "wps" | "wpt" => "wps",
        "apk" => "apk",
        "exe" | "dmg" => "exe",
        "key" => "key",
        _ => "unknown",
    }
}

/// 类型 → 图标底色（邮箱大师 AttachUtils.colorMap 原值，含大小写）。
pub fn kind_color(kind: &str) -> &'static str {
    match kind {
        "excel" => "#2EB257",
        "word" => "#3291FF",
        "ppt" => "#FF7421",
        "zip" => "#2EB257",
        "mail" => "#F59C00",
        "html" | "js" | "css" => "#0DC4D1",
        "font" => "#8F96A5",
        "pdf" => "#FF575C",
        "ps" => "#3291FF",
        "ai" => "#F59C00",
        "flash" => "#FF575C",
        "txt" => "#3291FF",
        "video" => "#0DC4D1",
        "music" => "#FF575C",
        "img" => "#617EFF",
        "dll" => "#8F96A5",
        "vcf" => "#3291FF",
        "ics" => "#2EB257",
        "wps" => "#3291FF",
        "apk" => "#2EB257",
        "exe" => "#8F96A5",
        "key" => "#F59C00",
        _ => "#8F96A5",
    }
}

/// 大小格式化（邮箱大师规则）：<1024 B 显示字节；否则 KB；≥1024 KB 转 MB。
/// KB/MB 均保留 1 位小数（`toFixed(1)`），如 `12.0 KB`、`1.5 MB`。
pub fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb >= 1024.0 {
        format!("{:.1} MB", kb / 1024.0)
    } else {
        format!("{kb:.1} KB")
    }
}

/// 超大附件过期倒计时格式化 —— 数值来源：邮箱大师 webui.zip `js/common.js`
/// AttachUtils（MECHANICS.md §5）。
///
/// 规则：`None`/0/负值 → `长期有效`（大师：`hugeExpiredDate == 0` =
/// 长期有效）；正值 → 非零最高两级「N天N小时后过期 / N小时N分后过期 /
/// N分N秒后过期 / N秒后过期」。
///
/// 显示框架先行：postium 无网易中转服务，附件模型暂无过期字段
/// （[`attachment_expiry_secs`] 现恒 None，UI 不渲染）；将来协议/服务端
/// 提供数据源时填入该函数即可点亮显示。
pub fn format_expiry(remaining_secs: Option<i64>) -> String {
    let Some(s) = remaining_secs else {
        return "长期有效".to_string();
    };
    if s <= 0 {
        return "长期有效".to_string();
    }
    let days = s / 86_400;
    let hours = (s % 86_400) / 3_600;
    let minutes = (s % 3_600) / 60;
    let secs = s % 60;
    if days > 0 {
        format!("{days}天{hours}小时后过期")
    } else if hours > 0 {
        format!("{hours}小时{minutes}分后过期")
    } else if minutes > 0 {
        format!("{minutes}分{secs}秒后过期")
    } else {
        format!("{secs}秒后过期")
    }
}

/// 附件过期剩余秒数数据源（邮箱大师 `hugeExpiredDate`，MECHANICS.md §5）。
///
/// postium 附件模型（`AttachmentDto`，attachment_service.rs）暂无该字段，
/// 当前恒返回 `None`（UI 相应不渲染过期条）。数据来源候选（将来任一可用
/// 即接入）：网易系超大附件中转 URL 的 `hugeExpiredDate` 私有字段（仅
/// 大师服务）；或 Content-Disposition 中的私有后缀。
pub fn attachment_expiry_secs(
    _att: &postium_core::service::attachment_service::AttachmentDto,
) -> Option<i64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_match_master_map() {
        assert_eq!(file_kind("报表.XLSX"), "excel");
        assert_eq!(file_kind("a.tar.gz"), "zip");
        assert_eq!(file_kind("b.PDF"), "pdf");
        assert_eq!(file_kind("photo.jpeg"), "img");
        assert_eq!(file_kind("noext"), "unknown");
    }

    #[test]
    fn colors_are_master_values() {
        assert_eq!(kind_color("pdf"), "#FF575C");
        assert_eq!(kind_color("word"), "#3291FF");
        assert_eq!(kind_color("ppt"), "#FF7421");
        assert_eq!(kind_color("unknown"), "#8F96A5");
    }

    #[test]
    fn size_formatting_follows_master() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(12 * 1024), "12.0 KB");
        assert_eq!(format_size(1536 * 1024), "1.5 MB");
    }

    #[test]
    fn expiry_formatting_follows_master() {
        // 0 / None / 负值 → 长期有效（大师：0 = 长期有效）
        assert_eq!(format_expiry(Some(0)), "长期有效");
        assert_eq!(format_expiry(None), "长期有效");
        assert_eq!(format_expiry(Some(-5)), "长期有效");
        // 逐级显示
        assert_eq!(format_expiry(Some(45)), "45秒后过期");
        assert_eq!(format_expiry(Some(125)), "2分5秒后过期");
        assert_eq!(format_expiry(Some(3_725)), "1小时2分后过期"); // 1h2m5s
        assert_eq!(format_expiry(Some(90061)), "1天1小时后过期"); // 1d1h1m1s
    }
}
