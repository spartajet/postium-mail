//! UI 偏好持久化（替代旧版 localStorage）：~/.postium/ui-state.json

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct UiState {
    pub theme: Option<String>, // "dark" | "light" | "system"
    pub lang: Option<String>,  // "zh" | "en"
    /// 已放行远程图片的发件人（归一化邮箱，见 [`normalize_sender`]）。
    /// 「加载图片」点击后按发件人记忆，后续该发件人的邮件自动放行
    /// （Gmail「始终显示来自此发件人的图片」语义，线上反馈①——此前
    /// 放行态随邮件装载生灭，每次打开都要重新点击）。
    pub remote_image_senders: Option<Vec<String>>,
}

impl UiState {
    /// 该发件人是否已放行远程图片（`sender` 须先经 [`normalize_sender`]）。
    pub fn allows_remote_images_from(&self, sender: &str) -> bool {
        self.remote_image_senders
            .as_ref()
            .is_some_and(|v| v.iter().any(|s| s == sender))
    }
}

fn path() -> std::path::PathBuf {
    crate::resolve_data_dir().join("ui-state.json")
}

pub fn load() -> UiState {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(state: &UiState) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path(), json);
    }
}

/// 发件人归一化键：小写；防御 `Name <a@b>` 形态取尖括号内地址；trim 空白。
pub fn normalize_sender(sender: &str) -> String {
    let s = sender.trim();
    let inner = s.rsplit_once('<').map(|(_, rest)| rest);
    let email = match inner {
        Some(rest) => rest.trim_end_matches('>'),
        None => s,
    };
    email.trim().to_ascii_lowercase()
}

/// 记住发件人已放行远程图片（读改写整份 ui-state.json——theme/lang 等其余
/// 字段原样保留；重复记忆幂等）。写盘失败静默（读侧 load 已宽容降级）。
pub fn remember_remote_image_sender(sender: &str) {
    let sender = normalize_sender(sender);
    if sender.is_empty() {
        return;
    }
    let mut state = load();
    let senders = state.remote_image_senders.get_or_insert_with(Vec::new);
    if !senders.iter().any(|s| *s == sender) {
        senders.push(sender);
        save(&state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_normalization() {
        assert_eq!(normalize_sender("A@B.example"), "a@b.example");
        assert_eq!(normalize_sender("  Aliyun <NOTICE@aliyun.com> "), "notice@aliyun.com");
        assert_eq!(normalize_sender("plain@x"), "plain@x");
        assert_eq!(normalize_sender(""), "");
        // 无右尖括号的残缺形态不误吞
        assert_eq!(normalize_sender("<broken@x"), "broken@x");
    }

    #[test]
    fn sender_allowlist_membership() {
        let mut state = UiState::default();
        assert!(!state.allows_remote_images_from("a@b"));
        state.remote_image_senders = Some(vec!["a@b".into(), "c@d".into()]);
        assert!(state.allows_remote_images_from("a@b"));
        assert!(!state.allows_remote_images_from("e@f"));
    }
}
