use crate::domain::sync::{InitialSyncRange, SyncWindow};

const DAY_SECONDS: i64 = 24 * 60 * 60;
const MONTH_SECONDS: i64 = 30 * DAY_SECONDS;

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

pub fn older_window_from_boundary(boundary: i64) -> SyncWindow {
    let boundary = normalize_to_imap_day_boundary(boundary);
    SyncWindow {
        start: Some(boundary - 3 * MONTH_SECONDS),
        end: Some(boundary),
    }
}

pub fn initialize_legacy_boundary(local_earliest_sent_at: Option<i64>, now: i64) -> i64 {
    normalize_to_imap_day_boundary(local_earliest_sent_at.unwrap_or(now - 3 * MONTH_SECONDS))
}

pub fn normalize_to_imap_day_boundary(timestamp: i64) -> i64 {
    timestamp.div_euclid(DAY_SECONDS) * DAY_SECONDS
}

#[cfg(test)]
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
