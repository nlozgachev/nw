use super::*;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

#[test]
fn history_range_round_trip() {
    for (input, expected) in [
        ("1m", HistoryRange::OneMonth),
        ("1M", HistoryRange::OneMonth),
        ("6M", HistoryRange::SixMonths),
        ("1y", HistoryRange::OneYear),
        ("5Y", HistoryRange::FiveYears),
        ("all", HistoryRange::All),
    ] {
        assert_eq!(input.parse::<HistoryRange>().unwrap(), expected);
    }
    assert!(matches!(
        "2W".parse::<HistoryRange>(),
        Err(NwError::InvalidHistoryRange(_))
    ));
    assert_eq!(HistoryRange::OneYear.to_string(), "1Y");
}

#[test]
fn legacy_json_deserializes_without_version_or_archived() {
    let json = r#"{
        "assets": [
            { "id": "vti", "name": "VTI", "category": "etf", "currency": "USD" }
        ],
        "snapshots": [
            {
                "date": "2025-02-28",
                "rates": { "EUR": 0.92 },
                "entries": [ { "asset_id": "vti", "value": 12500.5 } ]
            }
        ]
    }"#;

    let portfolio: Portfolio = serde_json::from_str(json).unwrap();
    assert_eq!(portfolio.version, CURRENT_SCHEMA_VERSION);
    assert!(!portfolio.assets[0].archived);
    assert_eq!(portfolio.snapshots[0].date, d(2025, 2, 28));
    assert_eq!(portfolio.snapshots[0].rates.get("EUR"), Some(&dec("0.92")));
    assert_eq!(portfolio.snapshots[0].entries[0].value, dec("12500.5"));
    portfolio.validate().unwrap();
}

#[test]
fn validate_rejects_usd_rate_non_positive_rate_and_duplicates() {
    let mut p = Portfolio::default();
    p.add_asset("vti", "VTI", "etf", "usd").unwrap();
    assert!(matches!(
        p.add_asset("vti", "Dup", "etf", "USD"),
        Err(NwError::DuplicateAssetId(_))
    ));

    let mut bad_rates = BTreeMap::new();
    bad_rates.insert("USD".to_string(), dec("1.0"));
    assert!(matches!(
        p.add_snapshot(Snapshot {
            date: d(2025, 1, 1),
            rates: bad_rates,
            entries: vec![],
        }),
        Err(NwError::UsdRateRejected)
    ));

    let mut zero_rates = BTreeMap::new();
    zero_rates.insert("EUR".to_string(), Decimal::ZERO);
    assert!(matches!(
        p.add_snapshot(Snapshot {
            date: d(2025, 1, 1),
            rates: zero_rates,
            entries: vec![],
        }),
        Err(NwError::InvalidRate { .. })
    ));

    let dup_entries = vec![
        SnapshotEntry {
            asset_id: "vti".to_string(),
            value: dec("10"),
        },
        SnapshotEntry {
            asset_id: "vti".to_string(),
            value: dec("20"),
        },
    ];
    assert!(matches!(
        p.add_snapshot(Snapshot {
            date: d(2025, 1, 1),
            rates: BTreeMap::new(),
            entries: dup_entries,
        }),
        Err(NwError::DuplicateSnapshotEntry { .. })
    ));
}

#[test]
fn validate_rejects_unsupported_schema_version() {
    let p = Portfolio {
        version: 99,
        assets: vec![],
        snapshots: vec![],
    };
    assert!(matches!(
        p.validate(),
        Err(NwError::UnsupportedSchemaVersion { found: 99, .. })
    ));
}

#[test]
fn edit_asset_blocks_currency_change_when_referenced_unless_forced() {
    let mut p = Portfolio::default();
    p.add_asset("sav", "Savings", "bank", "EUR").unwrap();
    let mut rates = BTreeMap::new();
    rates.insert("EUR".to_string(), dec("0.9"));
    p.add_snapshot(Snapshot {
        date: d(2025, 1, 1),
        rates,
        entries: vec![SnapshotEntry {
            asset_id: "sav".to_string(),
            value: dec("900"),
        }],
    })
    .unwrap();

    let err = p.edit_asset("sav", None, None, Some("USD"), false);
    assert!(matches!(
        err,
        Err(NwError::CurrencyChangeBlocked { count: 1, .. })
    ));

    let changed = p.edit_asset("sav", None, None, Some("USD"), true).unwrap();
    assert!(changed);
    assert_eq!(p.assets[0].currency, "USD");
}

#[test]
fn archive_and_remove_asset_work_properly() {
    let mut p = Portfolio::default();
    p.add_asset("vti", "VTI", "etf", "USD").unwrap();
    p.add_asset("eur", "EUR Cash", "cash", "EUR").unwrap();
    p.set_asset_archived("eur", true).unwrap();

    assert_eq!(p.active_assets().len(), 1);
    assert_eq!(p.active_non_usd_currencies(), Vec::<String>::new());
    assert!(matches!(
        p.set_asset_archived("eur", true),
        Err(NwError::AssetArchiveState { .. })
    ));

    p.add_snapshot(Snapshot {
        date: d(2025, 1, 1),
        rates: BTreeMap::new(),
        entries: vec![SnapshotEntry {
            asset_id: "vti".to_string(),
            value: dec("100"),
        }],
    })
    .unwrap();

    let count = p.remove_asset("vti", true).unwrap();
    assert_eq!(count, 1);
    assert_eq!(p.snapshots[0].entries, []);
}
