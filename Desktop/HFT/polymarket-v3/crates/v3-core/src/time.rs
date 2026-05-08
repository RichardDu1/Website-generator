use chrono::{DateTime, Duration, Timelike, Utc};

/// Calculate the next 15-minute window
pub fn next_15min_window() -> (DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();
    let minutes = now.minute();
    let next_quarter = ((minutes / 15) + 1) * 15;

    let open_time = if next_quarter >= 60 {
        now.with_minute(0).unwrap().with_second(0).unwrap() + Duration::hours(1)
    } else {
        now.with_minute(next_quarter).unwrap().with_second(0).unwrap()
    };

    let close_time = open_time + Duration::minutes(15);
    (open_time, close_time)
}

/// Calculate the current 15-minute window
pub fn current_15min_window() -> (DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();
    let minutes = now.minute();
    let current_quarter = (minutes / 15) * 15;

    let open_time = now.with_minute(current_quarter).unwrap().with_second(0).unwrap();
    let close_time = open_time + Duration::minutes(15);
    (open_time, close_time)
}

/// Check if we're within N seconds of market close
pub fn is_near_close(close_time: DateTime<Utc>, threshold_secs: i64) -> bool {
    let remaining = (close_time - Utc::now()).num_seconds();
    remaining <= threshold_secs && remaining > 0
}

/// Check if we're within N seconds of market open
pub fn is_near_open(open_time: DateTime<Utc>, threshold_secs: i64) -> bool {
    let elapsed = (Utc::now() - open_time).num_seconds();
    elapsed <= threshold_secs && elapsed >= 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_15min_windows() {
        let (open, close) = current_15min_window();
        assert_eq!((close - open).num_minutes(), 15);

        let (next_open, next_close) = next_15min_window();
        assert_eq!((next_close - next_open).num_minutes(), 15);
        assert!(next_open > open);
    }
}
