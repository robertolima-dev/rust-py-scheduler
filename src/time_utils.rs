use std::time::{SystemTime, UNIX_EPOCH};

/// Wall-clock seconds since the Unix epoch, as a string. This is the
/// Python-facing display counterpart to the `Instant`-based deadlines the
/// run loop actually schedules against (see `registry::RegisteredJob`):
/// `Instant` is monotonic but has no meaning outside the process, so it
/// can't be shown to a user, hence this separate, simple representation.
pub fn now_as_string() -> String {
    epoch_seconds().to_string()
}

/// Wall-clock seconds since the Unix epoch, `seconds_from_now` seconds in
/// the future, as a string.
pub fn future_as_string(seconds_from_now: u64) -> String {
    (epoch_seconds() + seconds_from_now).to_string()
}

fn epoch_seconds() -> u64 {
    // `unwrap_or_default()` rather than `.expect()`: this only fails if the
    // system clock is set before 1970, which would make the resulting
    // timestamp meaningless but is not a reason to panic a display-only
    // helper. Falling back to 0 keeps behavior total and non-panicking.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_as_string_is_a_parseable_unix_timestamp() {
        let value: u64 = now_as_string().parse().expect("must be a valid u64");
        assert!(value > 0);
    }

    #[test]
    fn future_as_string_is_later_than_now_as_string() {
        let now: u64 = now_as_string().parse().expect("must be a valid u64");
        let future: u64 = future_as_string(60).parse().expect("must be a valid u64");
        assert!(future >= now + 60);
    }
}
