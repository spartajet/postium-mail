//! 新邮件提示音 —— 对照网易邮箱大师：来新邮件时播放一声提示音，
//! 独立于系统通知（大师自带音频文件并在收信路径触发，MECHANICS.md §7）。
//!
//! Windows 用 winmm `PlaySoundA`（系统事件别名，无需自带 wav 文件、无新
//! 二进制依赖）；其他平台先留 no-op（等价行为可后续接 rodio）。
//!
//! 触发点：SyncStore 消费到 Completed 且本次同步有新邮件；手工全量同步
//! （首次拉历史）不响，只对「增量到达的新邮件」响——和大师「收到新信响
//! 一声」语义一致。
//!
//! 节流：5 秒静默期（服务器批量投递/多文件夹同轮完成时防连响）。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// 静默期：两次提示音之间至少间隔 5s（防批量同步连响）。
pub const MIN_INTERVAL: Duration = Duration::from_secs(5);

/// 异步播放标志（SND_ASYNC=1：立即返回不阻塞 UI 线程；
/// SND_ALIAS=0x10000：按系统事件名播放）。
#[cfg(target_os = "windows")]
const SND_ASYNC: u32 = 0x0001;
#[cfg(target_os = "windows")]
const SND_ALIAS: u32 = 0x1_0000;

/// 上次播放时间（Unix 毫秒；0 = 从未播过）。
static LAST_PLAYED_MS: AtomicU64 = AtomicU64::new(0);

#[cfg(target_os = "windows")]
#[link(name = "winmm")]
unsafe extern "system" {
    /// BOOL PlaySoundA(LPCSTR pszSound, HMODULE hmod, DWORD fdwSound)
    fn PlaySoundA(
        pszSound: *const std::ffi::c_char,
        hmod: *mut core::ffi::c_void,
        fdwSound: u32,
    ) -> i32;
}

/// 播放一声新邮件提示音（带 5s 节流；非 Windows 平台 no-op）。
pub fn play_new_mail_chime() {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // CAS 节流：抢不到更新权说明 5s 内已有一次播放，直接跳过
    let last = LAST_PLAYED_MS.load(Ordering::Relaxed);
    if last > 0 && now_ms.saturating_sub(last) < MIN_INTERVAL.as_millis() as u64 {
        return;
    }
    if LAST_PLAYED_MS
        .compare_exchange(last, now_ms, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return; // 并发竞争：另一线程刚播过
    }
    do_play();
}

#[cfg(target_os = "windows")]
fn do_play() {
    unsafe {
        // 系统事件别名 "SystemExclamation"（收信类提示，比 .Default 醒目）
        PlaySoundA(
            c"SystemExclamation".as_ptr(),
            core::ptr::null_mut(),
            SND_ASYNC | SND_ALIAS,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn do_play() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_interval_is_five_seconds() {
        assert_eq!(MIN_INTERVAL, Duration::from_secs(5));
    }
}
