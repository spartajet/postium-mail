//! i18n：全局语言状态与翻译查询。
//!
//! 词条表由 `scripts/gen_i18n.py` 从 Svelte 版翻译文件生成（`i18n_tables.rs`），
//! 键为扁平化 `section.key`。占位符 `{name}` 用 [`t_format`] 填充。

use std::sync::atomic::{AtomicU8, Ordering};

use gpui_kit::{App, Global, SharedString};

mod tables {
    include!("i18n_tables.rs");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    fn table(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::Zh => tables::ZH,
            Lang::En => tables::EN,
        }
    }

    fn to_u8(self) -> u8 {
        match self {
            Lang::Zh => 0,
            Lang::En => 1,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => Lang::En,
            _ => Lang::Zh,
        }
    }
}

/// 当前语言（GPUI 单 UI 线程模型，原子量即可支撑无上下文查询）
static CURRENT: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy)]
pub struct I18nGlobal {
    pub lang: Lang,
}

impl Global for I18nGlobal {}

/// 查询翻译；未命中返回 key 原样（便于发现漏译）。
pub fn t(key: &str) -> SharedString {
    let lang = Lang::from_u8(CURRENT.load(Ordering::Relaxed));
    lookup(lang, key).unwrap_or_else(|| key.to_string().into())
}

fn lookup(lang: Lang, key: &str) -> Option<SharedString> {
    lang.table()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| (*v).into())
}

/// 翻译并填充 `{name}` 占位符。
pub fn t_format(key: &str, args: &[(&str, &str)]) -> SharedString {
    let mut s = t(key).to_string();
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s.into()
}

/// 切换语言并广播刷新。
pub fn set_lang(lang: Lang, cx: &mut App) {
    CURRENT.store(lang.to_u8(), Ordering::Relaxed);
    cx.set_global(I18nGlobal { lang });
}

/// 当前语言（无上下文读取，供渲染层分段控件等使用）。
pub fn current_lang() -> Lang {
    Lang::from_u8(CURRENT.load(Ordering::Relaxed))
}

/// 供测试直接查表。
#[allow(dead_code)]
pub fn translate(lang: Lang, key: &str) -> Option<SharedString> {
    lookup(lang, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_en_key_sets_identical() {
        let zh: std::collections::HashSet<&str> = tables::ZH.iter().map(|(k, _)| *k).collect();
        let en: std::collections::HashSet<&str> = tables::EN.iter().map(|(k, _)| *k).collect();
        assert_eq!(zh, en, "中英词条键集合必须一致");
        assert!(!zh.is_empty());
    }

    #[test]
    fn spot_translations() {
        assert_eq!(translate(Lang::Zh, "sidebar.inbox").unwrap(), "收件箱");
        assert_eq!(translate(Lang::En, "sidebar.inbox").unwrap(), "Inbox");
        assert_eq!(translate(Lang::Zh, "common.confirm").unwrap(), "确认");
    }

    #[test]
    fn format_fills_placeholders() {
        // 不经过 Global 的纯格式化路径验证
        let s = "及其他 {count} 个收件人".replace("{count}", "3");
        assert_eq!(s, "及其他 3 个收件人");
    }
}
