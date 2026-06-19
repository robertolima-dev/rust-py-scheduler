use std::time::Duration;

use crate::errors::SchedulerError;

/// Parses strings like "10s", "5m" or "1h" into a `Duration`.
pub fn parse_interval(input: &str) -> Result<Duration, SchedulerError> {
    let trimmed = input.trim();

    if trimmed.len() < 2 {
        return Err(SchedulerError::InvalidInterval(input.to_string()));
    }

    let split_at = trimmed.len() - 1;
    let (amount_part, unit_part) = trimmed.split_at(split_at);

    let amount: u64 = amount_part
        .parse()
        .map_err(|_| SchedulerError::InvalidInterval(input.to_string()))?;

    if amount == 0 {
        return Err(SchedulerError::InvalidInterval(input.to_string()));
    }

    let seconds = match unit_part {
        "s" => amount,
        "m" => amount * 60,
        "h" => amount * 3600,
        _ => return Err(SchedulerError::InvalidInterval(input.to_string())),
    };

    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_seconds() {
        assert_eq!(parse_interval("10s").unwrap(), Duration::from_secs(10));
    }

    #[test]
    fn parses_minutes() {
        assert_eq!(parse_interval("5m").unwrap(), Duration::from_secs(5 * 60));
    }

    #[test]
    fn parses_hours() {
        assert_eq!(parse_interval("1h").unwrap(), Duration::from_secs(3600));
    }

    #[test]
    fn rejects_empty_string() {
        assert!(parse_interval("").is_err());
    }

    #[test]
    fn rejects_missing_unit() {
        assert!(parse_interval("10").is_err());
    }

    #[test]
    fn rejects_unknown_unit() {
        assert!(parse_interval("10x").is_err());
    }

    #[test]
    fn rejects_zero_amount() {
        assert!(parse_interval("0s").is_err());
    }

    #[test]
    fn rejects_negative_amount() {
        assert!(parse_interval("-5s").is_err());
    }

    #[test]
    fn rejects_non_numeric_amount() {
        assert!(parse_interval("abcs").is_err());
    }
}
