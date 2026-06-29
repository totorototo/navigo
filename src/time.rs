#[derive(Debug, PartialEq)]
pub enum TimeParseError {
    InvalidFormat,
    InvalidTimezone,
    InvalidNumber,
}

fn parse_i32(bytes: &[u8]) -> Result<i32, TimeParseError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(TimeParseError::InvalidNumber)
}

fn date_time_to_unix_seconds(
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
) -> i64 {
    let mut y = year as i64;
    let mut m = month as i64;
    if m <= 2 {
        y -= 1;
        m += 12;
    }
    let days = 365 * y + y / 4 - y / 100 + y / 400 + (153 * m - 457) / 5 + day as i64 - 719469;
    days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64
}

fn parse_timezone_offset(bytes: &[u8]) -> Result<i64, TimeParseError> {
    if bytes.len() == 1 && bytes[0] == b'Z' {
        return Ok(0);
    }
    if bytes.len() != 6 {
        return Err(TimeParseError::InvalidTimezone);
    }
    let sign: i64 = match bytes[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return Err(TimeParseError::InvalidTimezone),
    };
    if bytes[3] != b':' {
        return Err(TimeParseError::InvalidTimezone);
    }
    let hours = parse_i32(&bytes[1..3])? as i64;
    let minutes = parse_i32(&bytes[4..6])? as i64;
    Ok(-sign * (hours * 3600 + minutes * 60))
}

/// Parse an ISO 8601 timestamp string into a Unix epoch (seconds).
///
/// Supported formats:
/// - `2025-11-20T12:00:00Z`
/// - `2025-11-20T12:00:00+01:00`
/// - `2025-11-20T12:00:00-05:00`
pub fn parse_iso8601_to_epoch(s: &str) -> Result<i64, TimeParseError> {
    let b = s.as_bytes();
    if b.len() < 20 || b.len() > 25 {
        return Err(TimeParseError::InvalidFormat);
    }
    if b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return Err(TimeParseError::InvalidFormat);
    }
    let year = parse_i32(&b[0..4])?;
    let month = parse_i32(&b[5..7])?;
    let day = parse_i32(&b[8..10])?;
    let hour = parse_i32(&b[11..13])?;
    let minute = parse_i32(&b[14..16])?;
    let second = parse_i32(&b[17..19])?;
    let timestamp = date_time_to_unix_seconds(year, month, day, hour, minute, second);
    let tz_offset = parse_timezone_offset(&b[19..])?;
    Ok(timestamp + tz_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_timestamp() {
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20T12:00:00Z").unwrap(),
            1763640000
        );
    }

    #[test]
    fn unix_epoch() {
        assert_eq!(parse_iso8601_to_epoch("1970-01-01T00:00:00Z").unwrap(), 0);
    }

    #[test]
    fn positive_timezone_offset() {
        let utc = parse_iso8601_to_epoch("2025-11-20T12:00:00Z").unwrap();
        let cet = parse_iso8601_to_epoch("2025-11-20T12:00:00+01:00").unwrap();
        assert_eq!(cet, utc - 3600);
    }

    #[test]
    fn negative_timezone_offset() {
        let utc = parse_iso8601_to_epoch("2025-11-20T12:00:00Z").unwrap();
        let est = parse_iso8601_to_epoch("2025-11-20T12:00:00-05:00").unwrap();
        assert_eq!(est, utc + 18000);
    }

    #[test]
    fn same_moment_different_timezones() {
        let utc = parse_iso8601_to_epoch("2025-11-20T12:00:00Z").unwrap();
        let cet = parse_iso8601_to_epoch("2025-11-20T13:00:00+01:00").unwrap();
        let est = parse_iso8601_to_epoch("2025-11-20T07:00:00-05:00").unwrap();
        assert_eq!(utc, cet);
        assert_eq!(utc, est);
    }

    #[test]
    fn invalid_format_too_short() {
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20T12:00"),
            Err(TimeParseError::InvalidFormat)
        );
    }

    #[test]
    fn invalid_format_missing_t() {
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20 12:00:00Z"),
            Err(TimeParseError::InvalidFormat)
        );
    }

    #[test]
    fn year_2000() {
        assert_eq!(
            parse_iso8601_to_epoch("2000-01-01T00:00:00Z").unwrap(),
            946684800
        );
    }

    #[test]
    fn leap_year() {
        assert_eq!(
            parse_iso8601_to_epoch("2024-02-29T00:00:00Z").unwrap(),
            1709164800
        );
    }

    #[test]
    fn invalid_number_in_date_or_time() {
        assert_eq!(
            parse_iso8601_to_epoch("202a-11-20T12:00:00Z"),
            Err(TimeParseError::InvalidNumber)
        );
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-2aT12:00:00Z"),
            Err(TimeParseError::InvalidNumber)
        );
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20T12:0a:00Z"),
            Err(TimeParseError::InvalidNumber)
        );
    }

    #[test]
    fn invalid_timezone_shapes() {
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20T12:00:00X01:00"),
            Err(TimeParseError::InvalidTimezone)
        );
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20T12:00:00+0100"),
            Err(TimeParseError::InvalidTimezone)
        );
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20T12:00:00+ab:00"),
            Err(TimeParseError::InvalidNumber)
        );
        assert_eq!(
            parse_iso8601_to_epoch("2025-11-20T12:00:00+01:a0"),
            Err(TimeParseError::InvalidNumber)
        );
    }
}
