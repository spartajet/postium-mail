//! 轻量 CardDAV 客户端（spec §3.5）：标准 WebDAV 扩展协议。
//!
//! 覆盖 iCloud（应用专用密码）/ Nextcloud / Fastmail / Radicale：
//! - 发现链：PROPFIND current-user-principal → addressbook-home-set → 通讯录列表
//! - 拉取：REPORT addressbook-query（全量 href+etag）+ addressbook-multiget（vCard 正文）
//! - 推送：PUT（If-Match 可选；412 → Err(Conflict)）
//! - 删除：DELETE（404 视为成功）
//!
//! XML 解析为手写最小抽取（不引第三方 crate），健壮性目标是标准服务器报文。

use reqwest::{Client, Method, RequestBuilder, Url};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, ETAG};
use base64::Engine as _;

use crate::error::MailError;

/// 412 冲突专用错误（last-write-wins 决策由上层做）。
pub fn conflict() -> MailError {
    MailError::CardDavError("版本冲突（412）".into())
}

pub struct CardDavClient {
    http: Client,
    /// 用户输入的基地址（发现链起点）
    base: Url,
    /// 已解析的根（scheme://host[:port]，href 拼接用）
    root: Url,
    auth_header: String,
}

impl CardDavClient {
    pub fn new(url: &str, username: &str, password: &str) -> Result<Self, MailError> {
        let base = Url::parse(url).map_err(|e| MailError::CardDavError(format!("地址无效: {e}")))?;
        let mut root = base.clone();
        root.set_path("");
        root.set_query(None);
        root.set_fragment(None);
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
        );
        Ok(Self {
            http: Client::new(),
            base,
            root,
            auth_header,
        })
    }

    fn request(&self, method: Method, url: &str, depth: &str, body: &str) -> RequestBuilder {
        self.http
            .request(method, url)
            .header(AUTHORIZATION, &self.auth_header)
            .header(CONTENT_TYPE, "application/xml")
            .header("Depth", depth)
            .header("Brief", "t")
            .body(body.to_string())
    }

    /// href 可能是绝对 URL 或相对路径——统一解析到绝对。
    fn resolve(&self, href: &str) -> String {
        self.root.join(href).map(|u| u.to_string()).unwrap_or_else(|_| href.to_string())
    }

    async fn send_xml(&self, rb: RequestBuilder) -> Result<String, MailError> {
        let resp = rb
            .send()
            .await
            .map_err(|e| MailError::CardDavError(format!("网络错误: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(MailError::CardDavError(format!("HTTP {status}")));
        }
        Ok(resp
            .text()
            .await
            .map_err(|e| MailError::CardDavError(format!("读响应失败: {e}")))?)
    }

    /// 发现链：基地址 → principal → addressbook-home-set → 通讯录列表。
    pub async fn discover_addressbooks(&self) -> Result<Vec<(String, String)>, MailError> {
        // ① current-user-principal
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:"><D:prop><D:current-user-principal/></D:prop></D:propfind>"#;
        let xml = self
            .send_xml(self.request(Method::from_bytes(b"PROPFIND").unwrap(), self.base.as_str(), "0", body))
            .await?;
        let principal = extract_tag(&xml, "current-user-principal")
            .and_then(|seg| extract_tag(&seg, "href"))
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .ok_or_else(|| MailError::CardDavError("未发现 current-user-principal".into()))?;
        let principal_url = self.resolve(&principal);

        // ② addressbook-home-set
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav"><D:prop><C:addressbook-home-set/></D:prop></D:propfind>"#;
        let xml = self
            .send_xml(self.request(Method::from_bytes(b"PROPFIND").unwrap(), &principal_url, "0", body))
            .await?;
        let homeset = extract_tag(&xml, "addressbook-home-set")
            .and_then(|seg| extract_tag(&seg, "href"))
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .ok_or_else(|| MailError::CardDavError("未发现 addressbook-home-set".into()))?;
        let homeset_url = self.resolve(&homeset);

        // ③ Depth:1 列出通讯录（resourcetype 含 addressbook 的 collection）
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav"><D:prop><D:resourcetype/><D:displayname/></D:prop></D:propfind>"#;
        let xml = self
            .send_xml(self.request(Method::from_bytes(b"PROPFIND").unwrap(), &homeset_url, "1", body))
            .await?;
        Ok(parse_addressbooks(&xml))
    }

    /// REPORT addressbook-query：全量 (href, etag)。
    pub async fn list_entries(&self, ab_href: &str) -> Result<Vec<(String, String)>, MailError> {
        let url = self.resolve(ab_href);
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<C:addressbook-query xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav"><D:prop><D:getetag/></D:prop></C:addressbook-query>"#;
        let xml = self
            .send_xml(self.request(Method::from_bytes(b"REPORT").unwrap(), &url, "1", body))
            .await?;
        Ok(parse_entries(&xml))
    }

    /// REPORT addressbook-multiget：批量 (href, vCard 正文)。
    pub async fn multiget(&self, ab_href: &str, hrefs: &[String]) -> Result<Vec<(String, String)>, MailError> {
        if hrefs.is_empty() {
            return Ok(Vec::new());
        }
        let url = self.resolve(ab_href);
        let hrefs_xml = hrefs
            .iter()
            .map(|h| format!("<D:href>{}</D:href>", xml_escape(h)))
            .collect::<String>();
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<C:addressbook-multiget xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav"><D:prop><D:getetag/><D:address-data/></D:prop>{hrefs_xml}</C:addressbook-multiget>"#
        );
        let xml = self
            .send_xml(self.request(Method::from_bytes(b"REPORT").unwrap(), &url, "1", &body))
            .await?;
        Ok(parse_multiget(&xml))
    }

    /// PUT vCard；etag=Some 时带 If-Match（412 → Err(conflict)）。成功返回新 ETag。
    pub async fn put(&self, href: &str, vcard: &str, etag: Option<&str>) -> Result<Option<String>, MailError> {
        let url = self.resolve(href);
        let mut rb = self
            .http
            .put(&url)
            .header(AUTHORIZATION, &self.auth_header)
            .header(CONTENT_TYPE, "text/vcard; charset=utf-8")
            .body(vcard.to_string());
        if let Some(tag) = etag {
            rb = rb.header("If-Match", tag);
        }
        let resp = rb
            .send()
            .await
            .map_err(|e| MailError::CardDavError(format!("网络错误: {e}")))?;
        let status = resp.status();
        if status.as_u16() == 412 {
            return Err(conflict());
        }
        if !status.is_success() {
            return Err(MailError::CardDavError(format!("HTTP {status}")));
        }
        let headers = resp.headers();
        Ok(headers
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string))
    }

    /// DELETE；404 视为成功（服务端已不存在即达成目的）。
    pub async fn delete(&self, href: &str) -> Result<(), MailError> {
        let url = self.resolve(href);
        let resp = self
            .http
            .delete(&url)
            .header(AUTHORIZATION, &self.auth_header)
            .send()
            .await
            .map_err(|e| MailError::CardDavError(format!("网络错误: {e}")))?;
        let status = resp.status();
        if status.is_success() || status.as_u16() == 404 {
            Ok(())
        } else {
            Err(MailError::CardDavError(format!("HTTP {status}")))
        }
    }
}

// ── 最小 XML 抽取 ────────────────────────────────────────────────────────────

/// 越过注释/声明/CDATA：`j` 指向 `<` 之后的 `!` 或 `?`，返回标记结束之后的位置。
fn skip_markup(b: &[u8], j: usize) -> Option<usize> {
    if b.len() >= j + 8 && &b[j..j + 8] == b"![CDATA[" {
        let end = j + 8 + b[j + 8..].windows(3).position(|w| w == b"]]>")? + 3;
        Some(end)
    } else {
        Some(j + b[j..].iter().position(|&c| c == b'>')? + 1)
    }
}

/// 定位首个 `<(前缀:)?tag …>内容</(前缀:)?tag>` 的内容区间 [start, end)。
/// 命名空间前缀无关、大小写不敏感、同名嵌套计数配对；自闭合返回空区间。
fn find_tag_span(xml: &str, tag: &str) -> Option<(usize, usize)> {
    let b = xml.as_bytes();
    let n = b.len();
    let matches_name = |s: &str| {
        let local = s.rsplit(':').next().unwrap_or(s);
        local.eq_ignore_ascii_case(tag)
    };

    // ① 定位首个匹配的开标签
    let mut i = 0usize;
    let (content_start, self_closing) = loop {
        let lt = i + b[i..].iter().position(|&c| c == b'<')?;
        let mut j = lt + 1;
        if j < n && (b[j] == b'!' || b[j] == b'?') {
            i = skip_markup(b, j)?;
            continue;
        }
        let is_close = j < n && b[j] == b'/';
        if is_close {
            j += 1;
        }
        let name_end = j
            + b[j..]
                .iter()
                .position(|&c| matches!(c, b'>' | b' ' | b'/' | b'\t' | b'\r' | b'\n'))?;
        if !is_close && matches_name(&xml[j..name_end]) {
            let sc = b[name_end] == b'/';
            let after = if sc { name_end + 1 } else { name_end };
            let gt = after + b[after..].iter().position(|&c| c == b'>')?;
            break (gt + 1, sc);
        }
        i = name_end + b[name_end..].iter().position(|&c| c == b'>')? + 1;
    };
    if self_closing {
        return Some((content_start, content_start));
    }

    // ② 找配对闭标签（同名开/闭标签深度计数）
    let mut depth = 1usize;
    let mut j = content_start;
    loop {
        let lt = j + b[j..].iter().position(|&c| c == b'<')?;
        let mut k = lt + 1;
        if k < n && (b[k] == b'!' || b[k] == b'?') {
            j = skip_markup(b, k)?;
            continue;
        }
        let is_close = k < n && b[k] == b'/';
        if is_close {
            k += 1;
        }
        let name_end = k + b[k..].iter().position(|&c| matches!(c, b'>' | b' ' | b'/'))?;
        if matches_name(&xml[k..name_end]) {
            if is_close {
                depth -= 1;
                if depth == 0 {
                    return Some((content_start, lt));
                }
            } else if b[name_end] != b'/' {
                depth += 1;
            }
        }
        let after = if b[name_end] == b'/' { name_end + 1 } else { name_end };
        j = after + b[after..].iter().position(|&c| c == b'>')? + 1;
    }
}

/// 解码 XML 实体（数值实体 + 命名实体；`&amp;` 最后处理避免二次解码）。
fn xml_decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = s.to_string();
    for (ent, ch) in [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&apos;", "'")] {
        out = out.replace(ent, ch);
    }
    let mut res = String::with_capacity(out.len());
    let mut rest: &str = &out;
    while let Some(pos) = rest.find("&#") {
        res.push_str(&rest[..pos]);
        let after = &rest[pos + 2..];
        let (radix, digits) = match after.as_bytes().first() {
            Some(b'x') | Some(b'X') => (16, &after[1..]),
            _ => (10, after),
        };
        let len: usize = digits
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .map(char::len_utf8)
            .sum();
        if len > 0 && len <= 8 && digits[len..].starts_with(';') {
            if let Some(ch) = u32::from_str_radix(&digits[..len], radix).ok().and_then(char::from_u32) {
                res.push(ch);
                rest = &digits[len + 1..];
                continue;
            }
        }
        // 非法实体原样保留并越过，避免死循环
        res.push_str("&#");
        rest = after;
    }
    res.push_str(rest);
    res.replace("&amp;", "&")
}

/// 抽取首个匹配标签的内部内容。
fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    find_tag_span(xml, tag).map(|(s, e)| xml_decode(&xml[s..e]))
}

/// 顺序抽取所有匹配标签的内部内容。
fn extract_all(xml: &str, tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some((s, e)) = find_tag_span(rest, tag) {
        out.push(xml_decode(&rest[s..e]));
        rest = &rest[e..];
    }
    out
}

/// 抽取所有 `<response>...</response>` 段。
fn extract_responses(xml: &str) -> Vec<String> {
    extract_all(xml, "response")
}

fn parse_addressbooks(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for resp in extract_responses(xml) {
        let is_ab = extract_tag(&resp, "resourcetype")
            .map(|rt| rt.contains("addressbook"))
            .unwrap_or(false);
        if !is_ab {
            continue;
        }
        if let Some(href) = extract_tag(&resp, "href") {
            let name = extract_tag(&resp, "displayname").unwrap_or_default();
            out.push((href.trim().to_string(), name.trim().to_string()));
        }
    }
    out
}

fn parse_entries(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for resp in extract_responses(xml) {
        if let Some(href) = extract_tag(&resp, "href") {
            let etag = extract_tag(&resp, "getetag").unwrap_or_default();
            out.push((href.trim().to_string(), etag.trim().to_string()));
        }
    }
    out
}

fn parse_multiget(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for resp in extract_responses(xml) {
        if let Some(href) = extract_tag(&resp, "href") {
            let vcard = extract_tag(&resp, "address-data").unwrap_or_default();
            out.push((href.trim().to_string(), vcard));
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ── wiremock 测试 ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> CardDavClient {
        CardDavClient::new(&server.uri(), "user", "pass").unwrap()
    }

    #[tokio::test]
    async fn discover_full_chain() {
        let server = MockServer::start().await;

        // ① 基地址 → principal
        Mock::given(method("PROPFIND"))
            .and(path("/"))
            .and(header("Depth", "0"))
            .respond_with(ResponseTemplate::new(207).set_body_string(
                r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
 <D:response><D:href>/</D:href><D:propstat><D:prop>
  <D:current-user-principal><D:href>/principals/users/u1/</D:href></D:current-user-principal>
 </D:prop></D:propstat></D:response>
</D:multistatus>"#,
            ))
            .mount(&server)
            .await;

        // ② principal → homeset
        Mock::given(method("PROPFIND"))
            .and(path("/principals/users/u1/"))
            .respond_with(ResponseTemplate::new(207).set_body_string(
                r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">
 <D:response><D:href>/principals/users/u1/</D:href><D:propstat><D:prop>
  <C:addressbook-home-set><D:href>/addressbooks/u1/</D:href></C:addressbook-home-set>
 </D:prop></D:propstat></D:response>
</D:multistatus>"#,
            ))
            .mount(&server)
            .await;

        // ③ homeset Depth:1 → 两个 collection（其一为 addressbook）
        Mock::given(method("PROPFIND"))
            .and(path("/addressbooks/u1/"))
            .and(header("Depth", "1"))
            .respond_with(ResponseTemplate::new(207).set_body_string(
                r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">
 <D:response><D:href>/addressbooks/u1/</D:href><D:propstat><D:prop>
  <D:resourcetype><D:collection/></D:resourcetype><D:displayname>root</D:displayname>
 </D:prop></D:propstat></D:response>
 <D:response><D:href>/addressbooks/u1/main/</D:href><D:propstat><D:prop>
  <D:resourcetype><D:collection/><C:addressbook/></D:resourcetype><D:displayname>主通讯录</D:displayname>
 </D:prop></D:propstat></D:response>
</D:multistatus>"#,
            ))
            .mount(&server)
            .await;

        let abs = client(&server).discover_addressbooks().await.unwrap();
        assert_eq!(abs.len(), 1, "只有 addressbook collection 应入选: {abs:?}");
        assert_eq!(abs[0].0, "/addressbooks/u1/main/");
        assert_eq!(abs[0].1, "主通讯录");
    }

    #[tokio::test]
    async fn list_entries_parses_etags() {
        let server = MockServer::start().await;
        Mock::given(method("REPORT"))
            .and(path("/ab/"))
            .and(body_string_contains("addressbook-query"))
            .respond_with(ResponseTemplate::new(207).set_body_string(
                r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
 <D:response><D:href>/ab/a.vcf</D:href><D:propstat><D:prop><D:getetag>"e1"</D:getetag></D:prop></D:propstat></D:response>
 <D:response><D:href>/ab/b.vcf</D:href><D:propstat><D:prop><D:getetag>"e2"</D:getetag></D:prop></D:propstat></D:response>
</D:multistatus>"#,
            ))
            .mount(&server)
            .await;
        let entries = client(&server).list_entries("/ab/").await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("/ab/a.vcf".to_string(), "\"e1\"".to_string()));
    }

    #[tokio::test]
    async fn multiget_parses_vcards() {
        let server = MockServer::start().await;
        Mock::given(method("REPORT"))
            .and(path("/ab/"))
            .and(body_string_contains("addressbook-multiget"))
            .and(body_string_contains("/ab/a.vcf"))
            .respond_with(ResponseTemplate::new(207).set_body_string(
                r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:carddav">
 <D:response><D:href>/ab/a.vcf</D:href><D:propstat><D:prop><D:getetag>"e1"</D:getetag>
 <C:address-data>BEGIN:VCARD&#13;
VERSION:3.0&#13;
FN:张三&#13;
END:VCARD</C:address-data></D:prop></D:propstat></D:response>
</D:multistatus>"#,
            ))
            .mount(&server)
            .await;
        let cards = client(&server)
            .multiget("/ab/", &["/ab/a.vcf".to_string()])
            .await
            .unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].0, "/ab/a.vcf");
        assert!(cards[0].1.contains("BEGIN:VCARD"));
        assert!(cards[0].1.contains("张三"));
    }

    #[tokio::test]
    async fn put_with_and_without_if_match() {
        let server = MockServer::start().await;

        // 无 If-Match（新建）
        Mock::given(method("PUT"))
            .and(path("/ab/new.vcf"))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("ETag", "\"new-etag\""),
            )
            .expect(1)
            .mount(&server)
            .await;

        // 带 If-Match（更新）；校验头由独立 mock 断言
        Mock::given(method("PUT"))
            .and(path("/ab/x.vcf"))
            .and(header("If-Match", "\"old\""))
            .respond_with(
                ResponseTemplate::new(204)
                    .insert_header("ETag", "\"etag-2\""),
            )
            .expect(1)
            .mount(&server)
            .await;

        let c = client(&server);
        let etag = c.put("/ab/new.vcf", "BEGIN:VCARD\r\nEND:VCARD", None).await.unwrap();
        assert_eq!(etag.as_deref(), Some("\"new-etag\""));

        let etag = c.put("/ab/x.vcf", "BEGIN:VCARD\r\nEND:VCARD", Some("\"old\"")).await.unwrap();
        assert_eq!(etag.as_deref(), Some("\"etag-2\""));
    }

    #[tokio::test]
    async fn put_412_is_conflict_and_delete_404_ok() {
        let server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/ab/c.vcf"))
            .and(header("If-Match", "\"stale\""))
            .respond_with(ResponseTemplate::new(412))
            .mount(&server)
            .await;

        Mock::given(method("DELETE"))
            .and(path("/ab/gone.vcf"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let c = client(&server);
        let err = c.put("/ab/c.vcf", "x", Some("\"stale\"")).await.unwrap_err();
        assert!(
            matches!(err, MailError::CardDavError(ref m) if m.contains("412") || m.contains("冲突")),
            "应报冲突"
        );

        c.delete("/ab/gone.vcf").await.unwrap(); // 404 → Ok
    }

    #[test]
    fn extract_tag_basics() {
        let xml = r#"<D:multistatus><D:response><D:href>/a/</D:href></D:response></D:multistatus>"#;
        assert_eq!(extract_tag(xml, "href").as_deref(), Some("/a/"));
        assert_eq!(extract_tag(xml, "response").map(|r| extract_tag(&r, "href").unwrap()), Some("/a/".to_string()));
        assert_eq!(extract_tag(xml, "missing"), None);
        // 命名空间前缀无关
        let xml2 = r#"<x:prop><C:addressbook-home-set><D:href>/ab/</D:href></C:addressbook-home-set></x:prop>"#;
        assert_eq!(extract_tag(xml2, "addressbook-home-set").and_then(|s| extract_tag(&s, "href")).as_deref(), Some("/ab/"));
    }

    #[test]
    fn extract_tag_containers_and_entities() {
        // 嵌套容器标签：完整内部内容 + 多 response 顺序抽取
        let xml = r#"<D:multistatus><D:response><D:href>/ab/a.vcf</D:href><D:propstat><D:prop><D:getetag>"e1"</D:getetag></D:prop></D:propstat></D:response><D:response><D:href>/ab/b.vcf</D:href></D:response></D:multistatus>"#;
        let responses = extract_responses(xml);
        assert_eq!(responses.len(), 2);
        assert_eq!(extract_tag(&responses[0], "href").as_deref(), Some("/ab/a.vcf"));
        assert_eq!(extract_tag(&responses[1], "href").as_deref(), Some("/ab/b.vcf"));
        // 自闭合 → 空串
        assert_eq!(extract_tag("<D:collection/><D:href>x</D:href>", "collection").as_deref(), Some(""));
        // 实体解码（数值 + 命名）
        assert_eq!(extract_tag("<C:address-data>A&#13;B&amp;C</C:address-data>", "address-data").as_deref(), Some("A\rB&C"));
    }
}
