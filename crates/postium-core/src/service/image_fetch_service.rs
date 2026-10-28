//! 远程图片拉取服务（spec §4.4）：HTML 渲染器远程图片加载的后端入口。
//!
//! 薄封装：内存缓存（moka，按响应体字节权重、50MB 总预算）+
//! 委托 [`ImageFetcher`](crate::infrastructure::image_fetch::ImageFetcher)
//! 执行受限 HTTP 拉取（15s 超时 / 10MB 上限 / `image/*` Content-Type 校验）。
//!
//! 设计要点：
//! - 缓存键为图片 URL；权重为字节长度，总预算 50MB（moka 内置淘汰策略
//!   为 TinyLFU 而非严格 LRU，但满足"内存占用有界"的容量目标）
//! - 仅缓存成功结果（失败不缓存，下次访问自动重试）
//! - 未做并发去重：同 URL 并发首拉可能重复请求（UI 侧按邮件页串行触发，
//!   可接受；如需去重可换 moka `try_get_with`）

use std::sync::Arc;

use moka::future::Cache;

use crate::error::MailError;
use crate::infrastructure::image_fetch::ImageFetcher;

/// 缓存总预算（50 MB，按响应体字节权重计）。
const CACHE_BUDGET_BYTES: u64 = 50 * 1024 * 1024;

/// 远程图片拉取服务：缓存 + 受限 HTTP。
#[derive(Clone)]
pub struct ImageFetchService {
    fetcher: Arc<ImageFetcher>,
    cache: Cache<String, Arc<[u8]>>,
}

impl ImageFetchService {
    pub fn new() -> Self {
        Self {
            fetcher: Arc::new(ImageFetcher::new()),
            cache: Cache::builder()
                .weigher(|_key: &String, value: &Arc<[u8]>| value.len() as u32)
                .max_capacity(CACHE_BUDGET_BYTES)
                .build(),
        }
    }

    /// 拉取远程图片字节（带缓存）：命中缓存不发网络请求。
    pub async fn fetch(&self, url: &str) -> Result<Arc<[u8]>, MailError> {
        if let Some(bytes) = self.cache.get(url).await {
            return Ok(bytes);
        }
        let bytes = self.fetcher.fetch(url).await?;
        self.cache.insert(url.to_string(), bytes.clone()).await;
        Ok(bytes)
    }
}

impl Default for ImageFetchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// 伪 PNG 字节（首 4 字节魔数，其余填充）。
    fn png_bytes(len: usize) -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G'];
        v.resize(len, 0xAB);
        v
    }

    /// 200 + image/png 1KB → Ok 字节；第二次命中缓存不发请求（expect(1)）。
    #[tokio::test]
    async fn fetches_image_within_limits() {
        let server = MockServer::start().await;
        let uri = server.uri();
        Mock::given(method("GET"))
            .and(path("/img.png"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "image/png")
                    .set_body_bytes(png_bytes(1024)),
            )
            .expect(1)
            .mount(&server)
            .await;

        let svc = ImageFetchService::new();
        let first = svc.fetch(&format!("{uri}/img.png")).await.unwrap();
        assert_eq!(first.len(), 1024);
        assert_eq!(&first[..4], &[0x89, b'P', b'N', b'G']);

        // 第二次调用应命中缓存（expect(1) 在 server drop 时校验：多一次请求即失败）。
        let second = svc.fetch(&format!("{uri}/img.png")).await.unwrap();
        assert_eq!(first, second);
    }

    /// Content-Type: text/html → Err（image/ 前缀校验）。
    #[tokio::test]
    async fn rejects_non_image_content_type() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "text/html; charset=utf-8")
                    .set_body_bytes(b"<html></html>".to_vec()),
            )
            .mount(&server)
            .await;

        let svc = ImageFetchService::new();
        let err = svc.fetch(&server.uri()).await.unwrap_err();
        assert!(matches!(err, MailError::ImageFetchFailed(_)), "{err}");
        assert!(err.to_string().contains("Content-Type"), "{err}");
    }

    /// 11MB（Content-Length 预检路径）→ Err：预检超限即拒，不下载响应体。
    #[tokio::test]
    async fn rejects_oversize_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Type", "image/png")
                    .set_body_bytes(png_bytes(11 * 1024 * 1024)),
            )
            .mount(&server)
            .await;

        let svc = ImageFetchService::new();
        let err = svc.fetch(&server.uri()).await.unwrap_err();
        assert!(matches!(err, MailError::ImageFetchFailed(_)), "{err}");
        assert!(err.to_string().contains("Content-Length"), "{err}");
    }

    /// 流式截断路径：chunked 响应无 Content-Length（预检无从谈起），
    /// 累计超 10MB 即断连。wiremock 无法去掉 Content-Length，用原生 TCP 写。
    #[tokio::test]
    async fn rejects_oversize_streamed_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            // 读完请求头（读到空行分隔符为止）。
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = sock.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                req.extend_from_slice(&buf[..n]);
                if req.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            // chunked 响应：12 × 1MB（超 10MB 上限）。客户端超限断连时
            // 写入失败属预期，静默退出。
            let _ = sock
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nTransfer-Encoding: chunked\r\n\r\n",
                )
                .await;
            let data = vec![0xABu8; 1024 * 1024];
            for _ in 0..12 {
                let mut frame = format!("{:x}\r\n", data.len()).into_bytes();
                frame.extend_from_slice(&data);
                frame.extend_from_slice(b"\r\n");
                if sock.write_all(&frame).await.is_err() {
                    return;
                }
            }
            let _ = sock.write_all(b"0\r\n\r\n").await;
        });

        let svc = ImageFetchService::new();
        let err = svc
            .fetch(&format!("http://{addr}/img.png"))
            .await
            .unwrap_err();
        assert!(matches!(err, MailError::ImageFetchFailed(_)), "{err}");
        assert!(err.to_string().contains("字节上限"), "{err}");
    }
}
