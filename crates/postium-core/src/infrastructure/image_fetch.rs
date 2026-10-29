//! 远程图片受限 HTTP 拉取（spec §4.4 远程图片加载的基础设施层）。
//!
//! 职责单一：给定 URL，按安全限制拉取响应体字节——
//! - 15s 总超时（Client 级，覆盖连接建立到响应体读毕）
//! - 10MB 大小上限：Content-Length 预检 + 流式累计超限即断，双保险
//! - Content-Type 必须以 `image/` 前缀开头（大小写不敏感）
//!
//! 不含缓存：moka 内存缓存编排在 service 层
//! [`ImageFetchService`](crate::service::image_fetch_service::ImageFetchService)。

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};

use crate::error::MailError;

/// 响应体大小上限（10 MB）。
pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// 单次拉取总超时（15 s）。
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

/// 远程图片拉取器：受限 HTTP GET，不含缓存。
pub struct ImageFetcher {
    http: Client,
}

impl ImageFetcher {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(FETCH_TIMEOUT)
                .build()
                .expect("reqwest Client 构建不应失败"),
        }
    }

    /// 按安全限制拉取 `url` 指向的图片字节。
    ///
    /// 校验顺序：URL scheme → HTTP 状态 → Content-Type 前缀 →
    /// Content-Length 预检 → 流式累计上限。任一不过即返回
    /// [`MailError::ImageFetchFailed`]（URL 非法为 [`MailError::InvalidParam`]）。
    pub async fn fetch(&self, url: &str) -> Result<Arc<[u8]>, MailError> {
        let parsed =
            Url::parse(url).map_err(|e| MailError::InvalidParam(format!("图片地址无效: {e}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(MailError::InvalidParam(format!(
                "图片地址仅支持 http/https: {parsed}"
            )));
        }

        let resp = self
            .http
            .get(parsed)
            .send()
            .await
            .map_err(|e| MailError::ImageFetchFailed(format!("网络错误: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(MailError::ImageFetchFailed(format!("HTTP {status}")));
        }

        // Content-Type 前缀校验（大小写不敏感）。
        let content_type = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !content_type.starts_with("image/") {
            return Err(MailError::ImageFetchFailed(format!(
                "非图片 Content-Type: {content_type}"
            )));
        }

        // 双保险之一：Content-Length 预检（缺失或虚报时由流式累计兜底）。
        if let Some(len) = resp.content_length()
            && len > MAX_IMAGE_BYTES as u64
        {
            return Err(MailError::ImageFetchFailed(format!(
                "图片超过 {MAX_IMAGE_BYTES} 字节上限（Content-Length: {len}）"
            )));
        }

        // 双保险之二：流式累计，超限即中止（不信任 Content-Length）。
        let mut resp = resp;
        let mut body: Vec<u8> = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| MailError::ImageFetchFailed(format!("读响应体失败: {e}")))?
        {
            if body.len() + chunk.len() > MAX_IMAGE_BYTES {
                return Err(MailError::ImageFetchFailed(format!(
                    "图片超过 {MAX_IMAGE_BYTES} 字节上限（流式累计截断）"
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Arc::from(body))
    }
}

impl Default for ImageFetcher {
    fn default() -> Self {
        Self::new()
    }
}
