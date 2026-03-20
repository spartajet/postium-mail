//! IMAP IDLE 管理器
//!
//! 管理 IMAP IDLE 连接，处理服务器推送事件

use super::client::AsyncImapClient;
use super::types::{IdleEvent, IdleState, ReconnectConfig};
use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

/// IMAP IDLE 管理器
///
/// 由于 async-imap 0.11 不直接暴露 IDLE API，
/// 当前实现使用轮询降级策略。
///
/// ## 未来升级路径
///
/// 当 async-imap 支持 IDLE 或迁移到其他库时，
/// 可以无缝切换到真正的服务器推送。
pub struct ImapIdleManager {
    /// IMAP 客户端
    client: Arc<tokio::sync::Mutex<AsyncImapClient>>,

    /// 账号ID
    account_id: i32,

    /// 监听的文件夹
    folder: String,

    /// IDLE 状态
    state: Arc<RwLock<IdleState>>,

    /// 运行标志
    running: Arc<AtomicBool>,

    /// 事件发送通道
    event_tx: mpsc::UnboundedSender<IdleEvent>,

    /// 重连配置
    reconnect_config: ReconnectConfig,

    /// 轮询间隔（秒）- 当 IDLE 不可用时使用
    polling_interval_secs: u64,
}

impl ImapIdleManager {
    /// 创建新的 IDLE 管理器
    ///
    /// ## 参数
    ///
    /// - `client`: IMAP 客户端
    /// - `account_id`: 账号 ID
    /// - `folder`: 要监听的文件夹
    /// - `event_tx`: 事件发送通道
    /// - `polling_interval_secs`: 轮询间隔（秒），默认 300 秒（5 分钟）
    pub fn new(
        client: Arc<tokio::sync::Mutex<AsyncImapClient>>,
        account_id: i32,
        folder: String,
        event_tx: mpsc::UnboundedSender<IdleEvent>,
        polling_interval_secs: u64,
    ) -> Self {
        Self {
            client,
            account_id,
            folder,
            state: Arc::new(RwLock::new(IdleState::Disconnected)),
            running: Arc::new(AtomicBool::new(false)),
            event_tx,
            reconnect_config: ReconnectConfig::default(),
            polling_interval_secs,
        }
    }

    /// 创建带重连配置的 IDLE 管理器
    pub fn with_reconnect_config(
        client: Arc<tokio::sync::Mutex<AsyncImapClient>>,
        account_id: i32,
        folder: String,
        event_tx: mpsc::UnboundedSender<IdleEvent>,
        polling_interval_secs: u64,
        reconnect_config: ReconnectConfig,
    ) -> Self {
        Self {
            client,
            account_id,
            folder,
            state: Arc::new(RwLock::new(IdleState::Disconnected)),
            running: Arc::new(AtomicBool::new(false)),
            event_tx,
            reconnect_config,
            polling_interval_secs,
        }
    }

    /// 启动 IDLE 监听
    ///
    /// 由于 async-imap 0.11 限制，实际使用轮询策略。
    pub async fn start(&self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Err(anyhow!("IDLE 管理器已在运行"));
        }

        self.running.store(true, Ordering::Relaxed);
        *self.state.write().await = IdleState::Connected;

        let client = self.client.clone();
        let folder = self.folder.clone();
        let running = self.running.clone();
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();
        let reconnect_config = self.reconnect_config.clone();
        let polling_interval = self.polling_interval_secs;
        let account_id = self.account_id;

        tracing::info!(
            "🔄 启动 IDLE 监听: account_id={}, folder={}, 轮询间隔={}秒",
            account_id,
            folder,
            polling_interval
        );

        // 启动监听循环
        tokio::spawn(async move {
            let mut current_count = 0;
            let mut last_uid = 0;
            let mut reconnect_attempts = 0;

            while running.load(Ordering::Relaxed) {
                // 检查 IDLE 支持
                let mut client_guard = client.lock().await;
                let has_idle = client_guard.check_idle_support().await;

                match has_idle {
                    Ok(true) => {
                        // 服务器支持 IDLE
                        tracing::debug!("服务器支持 IDLE，尝试使用 IDLE API");

                        // 由于 async-imap 限制，降级到轮询
                        drop(client_guard);
                        *state.write().await = IdleState::IdleActive;

                        match Self::polling_check(
                            &client,
                            &folder,
                            &mut current_count,
                            &mut last_uid,
                            &event_tx,
                        )
                        .await
                        {
                            Ok(_) => {
                                reconnect_attempts = 0;
                            }
                            Err(e) => {
                                tracing::error!("轮询检查失败: {}", e);
                                if reconnect_config.enabled {
                                    reconnect_attempts += 1;
                                    if reconnect_attempts >= reconnect_config.max_attempts {
                                        tracing::error!("超过最大重试次数，停止监听");
                                        let _ = event_tx.send(IdleEvent::Disconnected);
                                        break;
                                    }
                                    // 指数退避
                                    let delay = Self::calculate_reconnect_delay(
                                        reconnect_attempts,
                                        &reconnect_config,
                                    );
                                    tracing::info!("等待 {} 秒后重连...", delay);
                                    *state.write().await = IdleState::Reconnecting;
                                    tokio::time::sleep(Duration::from_secs(delay)).await;
                                } else {
                                    let _ = event_tx.send(IdleEvent::Error(e.to_string()));
                                    break;
                                }
                            }
                        }
                    }
                    Ok(false) => {
                        // 服务器不支持 IDLE，使用轮询
                        tracing::debug!("服务器不支持 IDLE，使用轮询策略");
                        drop(client_guard);
                        *state.write().await = IdleState::IdleActive;

                        match Self::polling_check(
                            &client,
                            &folder,
                            &mut current_count,
                            &mut last_uid,
                            &event_tx,
                        )
                        .await
                        {
                            Ok(_) => {
                                reconnect_attempts = 0;
                            }
                            Err(e) => {
                                tracing::error!("轮询检查失败: {}", e);
                                let _ = event_tx.send(IdleEvent::Error(e.to_string()));
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        // 检查失败，可能连接断开
                        tracing::error!("检查 IDLE 支持失败: {}", e);
                        *state.write().await = IdleState::Disconnected;

                        if reconnect_config.enabled
                            && reconnect_attempts < reconnect_config.max_attempts
                        {
                            reconnect_attempts += 1;
                            let delay = Self::calculate_reconnect_delay(
                                reconnect_attempts,
                                &reconnect_config,
                            );
                            tracing::info!("等待 {} 秒后重连...", delay);
                            *state.write().await = IdleState::Reconnecting;
                            tokio::time::sleep(Duration::from_secs(delay)).await;
                            continue;
                        } else {
                            let _ = event_tx.send(IdleEvent::Disconnected);
                            break;
                        }
                    }
                }

                // 等待下次轮询
                if running.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_secs(polling_interval)).await;
                }
            }

            tracing::info!(
                "⏹ IDLE 监听已停止: account_id={}, folder={}",
                account_id,
                folder
            );
        });

        Ok(())
    }

    /// 停止 IDLE 监听
    pub async fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        *self.state.write().await = IdleState::Disconnected;
        tracing::info!("🛑 停止 IDLE 监听: account_id={}", self.account_id);
    }

    /// 获取当前状态
    pub async fn get_state(&self) -> IdleState {
        self.state.read().await.clone()
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    // ===== 内部方法 =====

    /// 轮询检查新邮件
    async fn polling_check(
        client: &Arc<tokio::sync::Mutex<AsyncImapClient>>,
        folder: &str,
        current_count: &mut usize,
        last_uid: &mut u32,
        event_tx: &mpsc::UnboundedSender<IdleEvent>,
    ) -> Result<()> {
        let mut client_guard = client.lock().await;

        // 方法 1: 检查邮件数量变化（轻量级）
        let (new_count, has_new) = client_guard
            .check_new_emails(folder, *current_count)
            .await?;

        if has_new {
            // 方法 2: 获取新邮件 UID 列表
            let new_uids = client_guard.polling_fallback(folder, *last_uid).await?;

            for uid in new_uids {
                let _ = event_tx.send(IdleEvent::NewEmail {
                    folder: folder.to_string(),
                    uid,
                });

                if uid > *last_uid {
                    *last_uid = uid;
                }
            }

            *current_count = new_count;
        }

        Ok(())
    }

    /// 计算重连延迟（指数退避）
    fn calculate_reconnect_delay(attempt: u32, config: &ReconnectConfig) -> u64 {
        let delay = (config.initial_delay_secs as f64
            * config.backoff_multiplier.powi(attempt as i32 - 1))
        .min(config.max_delay_secs as f64) as u64;
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_idle_event_creation() {
        let event = IdleEvent::NewEmail {
            folder: "INBOX".to_string(),
            uid: 123,
        };
        assert_eq!(
            event,
            IdleEvent::NewEmail {
                folder: "INBOX".to_string(),
                uid: 123,
            }
        );
    }

    #[test]
    fn test_idle_state_equality() {
        assert_eq!(IdleState::Disconnected, IdleState::Disconnected);
        assert_eq!(IdleState::Connected, IdleState::Connected);
        assert_ne!(IdleState::Disconnected, IdleState::IdleActive);
    }

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.initial_delay_secs, 5);
        assert_eq!(config.max_delay_secs, 300);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_reconnect_config_custom() {
        let config = ReconnectConfig {
            enabled: false,
            max_attempts: 5,
            initial_delay_secs: 10,
            max_delay_secs: 600,
            backoff_multiplier: 1.5,
        };
        assert!(!config.enabled);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay_secs, 10);
        assert_eq!(config.max_delay_secs, 600);
        assert_eq!(config.backoff_multiplier, 1.5);
    }

    #[test]
    fn test_calculate_reconnect_delay() {
        let config = ReconnectConfig::default();

        // 第1次重试: 5秒
        let delay1 = ImapIdleManager::calculate_reconnect_delay(1, &config);
        assert_eq!(delay1, 5);

        // 第2次重试: 10秒 (5 * 2^1)
        let delay2 = ImapIdleManager::calculate_reconnect_delay(2, &config);
        assert_eq!(delay2, 10);

        // 第3次重试: 20秒 (5 * 2^2)
        let delay3 = ImapIdleManager::calculate_reconnect_delay(3, &config);
        assert_eq!(delay3, 20);

        // 最大延迟不超过 max_delay_secs
        let delay_large = ImapIdleManager::calculate_reconnect_delay(100, &config);
        assert_eq!(delay_large, 300); // max_delay_secs
    }

    #[tokio::test]
    async fn test_idle_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        // 发送测试事件
        tx.send(IdleEvent::NewEmail {
            folder: "INBOX".to_string(),
            uid: 1,
        })
        .unwrap();

        tx.send(IdleEvent::FlagsChanged {
            folder: "INBOX".to_string(),
            uid: 1,
            flags: vec!["\\Seen".to_string()],
        })
        .unwrap();

        // 接收事件
        let event1 = rx.recv().await.unwrap();
        assert_eq!(
            event1,
            IdleEvent::NewEmail {
                folder: "INBOX".to_string(),
                uid: 1,
            }
        );

        let event2 = rx.recv().await.unwrap();
        assert_eq!(
            event2,
            IdleEvent::FlagsChanged {
                folder: "INBOX".to_string(),
                uid: 1,
                flags: vec!["\\Seen".to_string()],
            }
        );
    }
}
