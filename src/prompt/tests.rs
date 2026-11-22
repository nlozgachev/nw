use super::*;
use chrono::NaiveDate;
use std::io::Cursor;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn sample_asset(id: &str, name: &str, category: &str, currency: &str) -> Asset {
    Asset {
        id: id.to_string(),
        name: name.to_string(),
        category: category.to_string(),
        currency: currency.to_string(),
        archived: false,
    }
}

#[test]
fn parse_numeric_accepts_commas_underscores_and_rejects_non_finite() {
    assert_eq!(parse_numeric_input("12,500.50"), Some(dec("12500.50")));
    assert_eq!(parse_numeric_input("2_500_000"), Some(dec("2500000")));
    assert_eq!(parse_numeric_input("inf"), None);
    assert_eq!(parse_numeric_input("infinity"), None);
    assert_eq!(parse_numeric_input("NaN"), None);
    assert_eq!(parse_numeric_input(""), None);
}

#[test]
fn prompt_rates_aborts_on_eof_without_infinite_loop() {
    let mut reader = Cursor::new(b"");
    let mut writer = Vec::new();
    let err = prompt_rates(
        &mut reader,
        &mut writer,
        &["EUR".to_string()],
        None,
        None,
        false,
    )
    .unwrap_err();
    assert!(matches!(err, NwError::Aborted));
}

#[test]
fn prompt_rates_rejects_usd_and_invalid_inputs_then_accepts_valid() {
    let mut reader = Cursor::new(b"");
    let mut writer = Vec::new();
    assert!(matches!(
        prompt_rates(
            &mut reader,
            &mut writer,
            &["USD".to_string()],
            None,
            None,
            false
        ),
        Err(NwError::UsdRateRejected)
    ));

    let mut reader = Cursor::new(b"\ninf\n-1\n0\n0.92\n");
    let mut writer = Vec::new();
    let rates = prompt_rates(
        &mut reader,
        &mut writer,
        &["EUR".to_string()],
        None,
        None,
        false,
    )
    .unwrap();
    assert_eq!(rates.get("EUR"), Some(&dec("0.92")));
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Rate is required."));
    assert!(output.contains("Invalid number."));
    assert!(output.contains("Rate must be a positive number."));
}

#[test]
fn prompt_asset_values_handles_edit_clear_and_carry_forward() {
    let vti = sample_asset("vti", "VTI", "etf", "USD");
    let cash = sample_asset("cash", "Cash", "cash", "USD");
    let assets = vec![&vti, &cash];

    let mut existing = BTreeMap::new();
    existing.insert("vti".to_string(), dec("1000"));
    existing.insert("cash".to_string(), dec("500"));

    // Keep VTI via Enter, clear Cash via `-`
    let mut reader = Cursor::new(b"\n-\n");
    let mut writer = Vec::new();
    let entries = prompt_asset_values(
        &mut reader,
        &mut writer,
        &assets,
        Some(&existing),
        None,
        false,
    )
    .unwrap();
    assert_eq!(
        entries,
        vec![SnapshotEntry {
            asset_id: "vti".to_string(),
            value: dec("1000"),
        }]
    );
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("press Enter to keep existing, '-' to omit"));

    // Carry-forward from previous hints
    let mut reader = Cursor::new(b"\n1,250.75\n");
    let mut writer = Vec::new();
    let entries = prompt_asset_values(
        &mut reader,
        &mut writer,
        &assets,
        None,
        Some(&existing),
        true,
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].value, dec("1000"));
    assert_eq!(entries[1].value, dec("1250.75"));
}

#[test]
fn prompt_asset_values_aborts_on_eof() {
    let vti = sample_asset("vti", "VTI", "etf", "USD");
    let mut reader = Cursor::new(b"1000\n");
    let mut writer = Vec::new();
    let cash = sample_asset("cash", "Cash", "cash", "USD");
    let err = prompt_asset_values(&mut reader, &mut writer, &[&vti, &cash], None, None, false)
        .unwrap_err();
    assert!(matches!(err, NwError::Aborted));
}

#[test]
fn confirm_defaults_to_no_and_aborts_on_eof() {
    let mut writer = Vec::new();
    let mut empty_line = Cursor::new(b"\n");
    assert!(!confirm(&mut empty_line, &mut writer, "Sure? (y/N)").unwrap());

    let mut yes_line = Cursor::new(b"YES\n");
    assert!(confirm(&mut yes_line, &mut writer, "Sure? (y/N)").unwrap());

    let mut eof = Cursor::new(b"");
    assert!(matches!(
        confirm(&mut eof, &mut writer, "Sure? (y/N)"),
        Err(NwError::Aborted)
    ));
}

#[test]
fn format_snapshot_diff_summarizes_changes() {
    let assets = vec![
        sample_asset("vti", "VTI", "etf", "USD"),
        sample_asset("eur", "EUR Cash", "cash", "EUR"),
    ];
    let mut old_rates = BTreeMap::new();
    old_rates.insert("EUR".to_string(), dec("0.90"));
    let old = Snapshot {
        date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        rates: old_rates,
        entries: vec![SnapshotEntry {
            asset_id: "vti".to_string(),
            value: dec("1000"),
        }],
    };

    let mut new_rates = BTreeMap::new();
    new_rates.insert("EUR".to_string(), dec("0.92"));
    let new_entries = vec![SnapshotEntry {
        asset_id: "eur".to_string(),
        value: dec("500"),
    }];

    let diff = format_snapshot_diff(&old, &new_rates, &new_entries, &assets);
    assert!(diff.contains("rate EUR: 0.90 -> 0.92"));
    assert!(diff.contains("EUR Cash: (omitted) -> 500"));
    assert!(diff.contains("VTI: 1000 -> (omitted)"));
}
