//! 应用运行时设置 —— 数据目录下 `settings.json`。
//!
//! 对照网易邮箱大师的「服务端下发配置」做法（fnd_4：降级轮询间隔来自
//! 服务器配置，mastercore.dll `mm::PushService::StartPoll` @ 0x106701b0，
//! 二进制内无 poll_interval 本地常量）。postium 没有配置服务，等价实现：
//! 本地 JSON 配置文件，改文件即生效（下次启动 IDLE 循环读取）。
//!
//! 容错：文件缺失/损坏/字段缺省 → 全部回默认值，不报错不阻断启动；
//! 未知字段忽略（向前兼容）；值域钳制（轮询间隔 15s–3600s，防止误配成
//! 每秒轮询打爆服务器或半天不查）。

use std::path::{Path, PathBuf};
use std::time::Duration;

/// 降级轮询间隔默认值（秒）——大师侧等价兜底节奏（fnd_4）。
pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 90;
/// 轮询间隔下限（秒）：低于此值视为误配，钳制到下限。
pub const MIN_POLL_INTERVAL_SECS: u64 = 15;
/// 轮询间隔上限（秒）。
pub const MAX_POLL_INTERVAL_SECS: u64 = 3600;

/// 运行时设置（当前只有轮询间隔；后续设置项在此扩展）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// IDLE 降级轮询间隔（秒）。
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
}

fn default_poll_interval_secs() -> u64 {
    DEFAULT_POLL_INTERVAL_SECS
}

impl Default for Settings {
    fn default() -> Self {
        Self { poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS }
    }
}

impl Settings {
    /// 钳制到合法值域（写前/读后统一调用）。
    pub fn sanitized(mut self) -> Self {
        self.poll_interval_secs =
            self.poll_interval_secs.clamp(MIN_POLL_INTERVAL_SECS, MAX_POLL_INTERVAL_SECS);
        self
    }

    /// 轮询间隔（Duration 形式）。
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }
}

/// 设置文件路径：`<data_dir>/settings.json`。
pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("settings.json")
}

/// 读取设置；文件缺失/损坏 → 默认值（并记 warn 日志）。
pub fn load(data_dir: &Path) -> Settings {
    let path = settings_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Settings>(&text) {
            Ok(s) => s.sanitized(),
            Err(e) => {
                tracing::warn!("settings.json 解析失败，使用默认设置: {e}");
                Settings::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(e) => {
            tracing::warn!("settings.json 读取失败，使用默认设置: {e}");
            Settings::default()
        }
    }
}

/// 写入设置（原子写：先写临时文件再 rename）；失败仅记日志。
pub fn save(data_dir: &Path, settings: &Settings) {
    let path = settings_path(data_dir);
    let settings = settings.clone().sanitized();
    match serde_json::to_string_pretty(&settings) {
        Ok(text) => {
            let tmp = path.with_extension("json.tmp");
            if let Err(e) = std::fs::write(&tmp, text)
                .and_then(|_| std::fs::rename(&tmp, &path))
            {
                tracing::warn!("settings.json 写入失败: {e}");
            }
        }
        Err(e) => tracing::warn!("settings.json 序列化失败: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_90s_and_sanitized() {
        assert_eq!(Settings::default().poll_interval(), Duration::from_secs(90));
        // 过小/过大钳制
        assert_eq!(
            Settings { poll_interval_secs: 1 }.sanitized().poll_interval_secs,
            MIN_POLL_INTERVAL_SECS
        );
        assert_eq!(
            Settings { poll_interval_secs: 99_999 }.sanitized().poll_interval_secs,
            MAX_POLL_INTERVAL_SECS
        );
    }

    #[test]
    fn load_missing_file_gives_default() {
        let dir = std::env::temp_dir().join(format!("postium-settings-test-{}", std::process::id()));
        // 不创建目录 → 文件必然缺失
        let s = load(&dir);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn load_save_roundtrip() {
        let dir = std::env::temp_dir().join(format!("postium-settings-rt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Settings { poll_interval_secs: 120 };
        save(&dir, &s);
        assert_eq!(load(&dir).poll_interval_secs, 120);
        // 写入值越界 → 读回被钳制
        save(&dir, &Settings { poll_interval_secs: 2 });
        assert_eq!(load(&dir).poll_interval_secs, MIN_POLL_INTERVAL_SECS);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupted_file_gives_default() {
        let dir = std::env::temp_dir().join(format!("postium-settings-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(settings_path(&dir), "{ not json").unwrap();
        assert_eq!(load(&dir), Settings::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_fields_ignored() {
        let dir = std::env::temp_dir().join(format!("postium-settings-unk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            settings_path(&dir),
            r#"{"poll_interval_secs": 45, "future_option": true}"#,
        )
        .unwrap();
        assert_eq!(load(&dir).poll_interval_secs, 45);
        std::fs::remove_dir_all(&dir).ok();
    }
}
