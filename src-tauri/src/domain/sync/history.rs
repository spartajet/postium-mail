//!
//! # IMAP 同步时间窗计算 (IMAP Sync Time-Window Math)
//!
//! 本模块负责将用户语义的同步范围（如「最近一周」「最近一个月」）转换成
//! 具体的 [`SyncWindow`] Unix 秒级时间戳区间，供 IMAP 搜索使用。
//!
//! 主要职责：
//! - 计算初始同步的起始时间（基于用户选择的 [`InitialSyncRange`]）
//! - 计算「向更早历史翻页」时的分页窗口
//! - 将任意时间戳归一化到当天 00:00 UTC，以适配 IMAP 日期搜索语义
//!

use crate::domain::sync::{InitialSyncRange, SyncWindow};

/// 一天的秒数（24 小时）。
///
/// 作为所有基于天/月的时间窗计算的基本单位，
/// 与 Unix 时间戳（秒）的天然粒度保持一致。
const DAY_SECONDS: i64 = 24 * 60 * 60;

/// 一个月（按 30 天）的秒数。
///
/// 采用 30 天作为「月」的近似值，而非日历月（28~31 天），
/// 目的是让 MONTH_SECONDS 成为一个固定常量，便于窗口的加减与对齐计算，
/// 同时保证各档位范围相对均匀，避免月份长度不一带来的边界偏差。
const MONTH_SECONDS: i64 = 30 * DAY_SECONDS;

/// 根据用户选择的初始同步范围计算同步时间窗口。
///
/// 初始同步时，根据 [`InitialSyncRange`] 往前推算起始时间戳，
/// `end` 始终为 `None`（即同步到最新）。
///
/// # 参数
///
/// - `range`: 用户选择的初始同步范围档位（周/月/季/年/全部）
/// - `now`: 当前时间戳（Unix 秒），用作窗口的右边界参考点
///
/// # 返回
///
/// 返回 [`SyncWindow`]，其中 `start` 为起始时间戳，
/// `All` 档位时 `start` 为 `None` 表示不设下限；`end` 恒为 `None`。
pub fn window_for_initial_range(range: InitialSyncRange, now: i64) -> SyncWindow {
    let start = match range {
        InitialSyncRange::Week => Some(now - 7 * DAY_SECONDS),
        InitialSyncRange::Month => Some(now - MONTH_SECONDS),
        InitialSyncRange::ThreeMonths => Some(now - 3 * MONTH_SECONDS),
        InitialSyncRange::Year => Some(now - 365 * DAY_SECONDS),
        InitialSyncRange::All => None,
    };

    SyncWindow { start, end: None }
}

/// 根据「更早历史」的当前边界计算向更早翻页的时间窗口。
///
/// 用户在拉取更早的邮件时，以 `boundary` 为上界（归一化到当天 00:00 UTC），
/// 再往前推 3 个月作为新窗口的下界。
///
/// # 参数
///
/// - `boundary`: 当前已同步区域的最早时间戳（Unix 秒），不要求已归一化
///
/// # 返回
///
/// 返回 [`SyncWindow`]，`start` 为 `boundary` 前推 3 个月（归一化后），
/// `end` 为归一化后的 `boundary`（即当天 00:00 UTC，作为 IMAP `BEFORE` 的排他上界）。
pub fn older_window_from_boundary(boundary: i64) -> SyncWindow {
    let boundary = normalize_to_imap_day_boundary(boundary);
    SyncWindow {
        start: Some(boundary - 3 * MONTH_SECONDS),
        end: Some(boundary),
    }
}

/// 初始化「更早历史」翻页用的初始边界时间戳。
///
/// 优先采用本地已存在邮件中最早一封的发送时间作为边界，
/// 若本地无邮件（`None`）则默认回退到当前时间前 3 个月。
/// 最终结果都会被归一化到当天 00:00 UTC。
///
/// # 参数
///
/// - `local_earliest_sent_at`: 本地已存邮件中最早一封的发送时间戳（Unix 秒），
///   `None` 表示本地尚无邮件
/// - `now`: 当前时间戳（Unix 秒），仅在本地无邮件时用于计算默认边界
///
/// # 返回
///
/// 归一化后的边界时间戳（当天 00:00 UTC）。
pub fn initialize_legacy_boundary(local_earliest_sent_at: Option<i64>, now: i64) -> i64 {
    normalize_to_imap_day_boundary(local_earliest_sent_at.unwrap_or(now - 3 * MONTH_SECONDS))
}

/// 将时间戳归一化到当天 00:00 UTC（当天起始）。
///
/// # 为什么需要归一化
///
/// IMAP 的 `SEARCH` 命令在按日期过滤时（如 `SINCE`、`BEFORE`）只精确到「天」，
/// 不支持按「时/分/秒」过滤。若直接把带有时分秒的时间戳用作边界，
/// 会与服务器按整天匹配的行为产生不可预期的偏差（例如略掉当天本应包含的邮件）。
///
/// 因此在把时间戳传入 IMAP 搜索条件之前，必须先将其向下对齐到
/// 当天 00:00 UTC，确保「该天内的所有邮件」都被正确纳入窗口。
///
/// # 实现
///
/// 利用 `div_euclid` 向下取整到天的秒数倍数，避免负时间戳（1970 年前）时
/// 取整方向出错；再乘回 `DAY_SECONDS` 即得到当天 00:00 UTC。
pub fn normalize_to_imap_day_boundary(timestamp: i64) -> i64 {
    timestamp.div_euclid(DAY_SECONDS) * DAY_SECONDS
}

#[cfg(test)]
///
/// # 单元测试
///
/// 覆盖各项时间窗计算逻辑：
/// - 各 [`InitialSyncRange`] 档位到窗口的正确换算
/// - 「更早历史」翻页窗口的边界与跨度
/// - `initialize_legacy_boundary` 的优先级、归一化与默认回退行为
mod tests {
    use super::*;

    #[test]
    fn initial_sync_range_should_convert_week_to_expected_window() {
        let now = 1_700_000_000;

        assert_eq!(
            window_for_initial_range(InitialSyncRange::Week, now),
            SyncWindow {
                start: Some(now - 7 * 24 * 60 * 60),
                end: None,
            },
        );
    }

    #[test]
    fn initial_sync_range_should_convert_month_to_expected_window() {
        let now = 1_700_000_000;

        assert_eq!(
            window_for_initial_range(InitialSyncRange::Month, now),
            SyncWindow {
                start: Some(now - 30 * 24 * 60 * 60),
                end: None,
            },
        );
    }

    #[test]
    fn initial_sync_range_should_convert_three_months_to_expected_window() {
        let now = 1_700_000_000;

        assert_eq!(
            window_for_initial_range(InitialSyncRange::ThreeMonths, now),
            SyncWindow {
                start: Some(now - 90 * 24 * 60 * 60),
                end: None,
            },
        );
    }

    #[test]
    fn initial_sync_range_should_convert_year_to_expected_window() {
        let now = 1_700_000_000;

        assert_eq!(
            window_for_initial_range(InitialSyncRange::Year, now),
            SyncWindow {
                start: Some(now - 365 * 24 * 60 * 60),
                end: None,
            },
        );
    }

    #[test]
    fn initial_sync_range_should_convert_all_to_expected_window() {
        let now = 1_700_000_000;

        assert_eq!(
            window_for_initial_range(InitialSyncRange::All, now),
            SyncWindow {
                start: None,
                end: None,
            },
        );
    }

    #[test]
    fn older_sync_window_should_move_boundary_back_three_months() {
        let boundary = 1_700_000_000;
        let window = older_window_from_boundary(boundary);
        let normalized_boundary = normalize_to_imap_day_boundary(boundary);

        assert_eq!(window.end, Some(normalized_boundary));
        assert_eq!(window.start, Some(normalized_boundary - 90 * 24 * 60 * 60));
    }

    #[test]
    fn legacy_boundary_should_prefer_local_earliest_email() {
        let now = 1_700_000_000;
        let local_earliest = 1_600_000_000;
        let boundary = initialize_legacy_boundary(Some(local_earliest), now);

        assert_eq!(boundary, normalize_to_imap_day_boundary(local_earliest));
    }

    #[test]
    fn legacy_boundary_should_normalize_non_midnight_timestamp_to_day_start() {
        let now = 1_700_000_000;
        let non_midnight = 1_600_027_199; // 2020-09-13 19:59:59 UTC
        let boundary = initialize_legacy_boundary(Some(non_midnight), now);

        assert_eq!(boundary, 1_599_955_200); // 2020-09-13 00:00:00 UTC
    }

    #[test]
    fn legacy_boundary_should_default_to_three_months_when_folder_empty() {
        let now = 1_700_000_000;
        let boundary = initialize_legacy_boundary(None, now);
        let default_boundary = now - 90 * 24 * 60 * 60;

        assert_eq!(boundary, normalize_to_imap_day_boundary(default_boundary));
    }
}
