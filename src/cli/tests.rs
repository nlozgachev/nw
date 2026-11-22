use super::*;

#[test]
fn parse_iso_date_requires_strict_zero_padded_format() {
    assert_eq!(
        parse_iso_date("2025-06-01").unwrap(),
        NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()
    );
    assert!(parse_iso_date("2025-6-1").is_err());
    assert!(parse_iso_date("2025-02-29").is_err());
}
