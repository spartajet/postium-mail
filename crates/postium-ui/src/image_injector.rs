//! 邮件正文图片注入（设计定案：内联 cid → 本地 data URI；远程图按策略处理）。

use base64::Engine;
use postium_core::service::attachment_service::InlineAttachmentDto;

/// 远程图片策略（隐私：默认屏蔽防已读追踪）
#[derive(Clone, Copy, PartialEq)]
pub enum RemoteImagePolicy {
    /// 删除远程 img（显示 alt 文本占位）
    Block,
    /// 保留远程 URL（渲染器不会自动加载，占位；后续版本走后端下载转 data URI）
    Keep,
}

/// 把邮件 HTML 中的图片引用替换为可渲染形式。
///
/// - `src="cid:xxx"` → 读本地缓存文件 → `data:<mime>;base64,...`
/// - `src="http(s)://..."` → 按 `policy` 处理
pub fn inject_images(html: &str, inline: &[InlineAttachmentDto], policy: RemoteImagePolicy) -> String {
    let mut out = html.to_string();

    for att in inline {
        let cid = normalize_cid(&att.content_id);
        let path = std::path::Path::new(&att.url);
        // 读取文件转 data URI（失败则删除该 img）
        let data_uri = std::fs::read(path).ok().map(|bytes| {
            let mime = mime_from_ext(path);
            format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
        });
        let replacement = match &data_uri {
            Some(uri) => format!(r#"src="{uri}""#),
            None => r#"alt="[图片不可用]""#.to_string(),
        };
        out = replace_cid_src(&out, &cid, &replacement);
    }

    match policy {
        RemoteImagePolicy::Block => remove_remote_imgs(&out),
        RemoteImagePolicy::Keep => out,
    }
}

/// cid 可能带尖括号（`<abc@def>`），匹配时统一去括号。
fn normalize_cid(cid: &str) -> String {
    cid.trim().trim_start_matches('<').trim_end_matches('>').to_string()
}

/// 替换 `src="cid:xxx"`（容忍单双引号与 cid: 大小写）。
fn replace_cid_src(html: &str, cid: &str, replacement: &str) -> String {
    for (q_open, q_close) in [("\"", "\""), ("'", "'")] {
        let patterns = [
            format!("src={q_open}cid:{cid}{q_close}"),
            format!("SRC={q_open}cid:{cid}{q_close}"),
            format!("src={q_open}CID:{cid}{q_close}"),
        ];
        let mut out = html.to_string();
        for p in &patterns {
            if out.contains(p.as_str()) {
                out = out.replace(p.as_str(), replacement);
            }
        }
        if out != html {
            return out;
        }
    }
    html.to_string()
}

/// 删除远程图（保留 alt 文本）。
fn remove_remote_imgs(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("<img") {
        let Some(end) = rest[start..].find('>') else { break };
        let tag = &rest[start..start + end + 1];
        let is_remote = tag.contains("src=\"http://") || tag.contains("src=\"https://")
            || tag.contains("src='http://") || tag.contains("src='https://");
        out.push_str(&rest[..start]);
        if is_remote {
            let alt = extract_attr(tag, "alt").unwrap_or_else(|| "远程图片已屏蔽".to_string());
            out.push_str(&format!("<span>[{alt}]</span>"));
        } else {
            out.push_str(tag);
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    // 模式 `name="`，字节长 name.len()+2（= 与引号）
    let idx = tag.find(&format!("{name}=\""))?;
    let rest = &tag[idx + name.len() + 2..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn mime_from_ext(path: &std::path::Path) -> String {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()) {
        Some(e) if e == "png" => "image/png",
        Some(e) if e == "jpg" || e == "jpeg" => "image/jpeg",
        Some(e) if e == "gif" => "image/gif",
        Some(e) if e == "webp" => "image/webp",
        Some(e) if e == "svg" => "image/svg+xml",
        Some(e) if e == "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_images_blocked_with_alt() {
        let html = r#"<p>hi</p><img src="https://x.test/a.png" alt="封面">"#;
        let out = inject_images(html, &[], RemoteImagePolicy::Block);
        assert!(out.contains("[封面]"), "{out}");
        assert!(!out.contains("https://x.test"), "{out}");
    }

    #[test]
    fn non_remote_images_kept_when_blocking() {
        let html = r#"<img src="data:image/png;base64,AAAA">"#;
        let out = inject_images(html, &[], RemoteImagePolicy::Block);
        assert!(out.contains("data:image/png"), "{out}");
    }

    #[test]
    fn cid_replacement_patterns() {
        let out = replace_cid_src(
            r#"<img src="cid:abc@def" width="10">"#,
            "abc@def",
            r#"src="data:image/png;base64,QQ==" "#,
        );
        assert!(out.contains("data:image/png;base64,QQ=="), "{out}");
        // 带尖括号的 cid 归一化
        assert_eq!(normalize_cid("<abc@def>"), "abc@def");
    }
}
