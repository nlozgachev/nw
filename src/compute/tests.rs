use super::*;
use crate::model::SnapshotEntry;
use std::str::FromStr;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn make_rates(pairs: &[(&str, &str)]) -> BTreeMap<String, Decimal> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), dec(v)))
        .collect()
}

fn make_snapshot(date: NaiveDate) -> Snapshot {
    Snapshot {
        date,
        rates: BTreeMap::new(),
        entries: Vec::new(),
    }
}

#[test]
fn to_usd_passthrough_and_foreign_conversion() {
    let rates = make_rates(&[("EUR", "0.92")]);
    assert_eq!(to_usd(dec("1000"), "USD", &rates).unwrap(), dec("1000"));
    assert_eq!(to_usd(dec("1000"), "usd", &rates).unwrap(), dec("1000"));
    let converted = to_usd(dec("800"), "EUR", &rates).unwrap();
    assert!((converted - dec("869.5652")).abs() < dec("0.001"));
}

#[test]
fn to_usd_rejects_missing_zero_and_negative_rates() {
    assert!(matches!(
        to_usd(dec("100"), "EUR", &BTreeMap::new()),
        Err(NwError::RateMissing(_))
    ));
    assert!(matches!(
        to_usd(dec("100"), "EUR", &make_rates(&[("EUR", "0")])),
        Err(NwError::InvalidRate { .. })
    ));
    assert!(matches!(
        to_usd(dec("100"), "EUR", &make_rates(&[("EUR", "-1.2")])),
        Err(NwError::InvalidRate { .. })
    ));
}

#[test]
fn compute_change_positive_negative_and_from_zero() {
    let (cu, cp) = compute_change(dec("42300"), dec("45100"));
    assert_eq!(cu, dec("2800"));
    assert!((cp.unwrap() - dec("6.619")).abs() < dec("0.01"));

    let (cu, cp) = compute_change(dec("45100"), dec("43800"));
    assert_eq!(cu, dec("-1300"));
    assert!((cp.unwrap() - dec("-2.882")).abs() < dec("0.01"));

    let (cu, cp) = compute_change(Decimal::ZERO, dec("100"));
    assert_eq!(cu, dec("100"));
    assert_eq!(cp, None);
}

#[test]
fn filter_by_range_standard_and_leap_year_boundaries() {
    let snapshots = vec![
        make_snapshot(d(2020, 1, 1)),
        make_snapshot(d(2023, 2, 28)),
        make_snapshot(d(2024, 2, 29)),
        make_snapshot(d(2024, 3, 1)),
        make_snapshot(d(2024, 8, 28)),
        make_snapshot(d(2025, 1, 15)),
        make_snapshot(d(2025, 2, 10)),
        make_snapshot(d(2025, 2, 28)),
    ];

    assert_eq!(
        filter_by_range(&snapshots, HistoryRange::All, d(2025, 2, 28)).len(),
        8
    );

    let r1m = filter_by_range(&snapshots, HistoryRange::OneMonth, d(2025, 2, 28));
    assert_eq!(
        r1m.iter().map(|s| s.date).collect::<Vec<_>>(),
        vec![d(2025, 2, 10), d(2025, 2, 28)]
    );

    let r6m = filter_by_range(&snapshots, HistoryRange::SixMonths, d(2025, 2, 28));
    assert_eq!(r6m.first().unwrap().date, d(2024, 8, 28));

    let r1y = filter_by_range(&snapshots, HistoryRange::OneYear, d(2025, 2, 28));
    assert_eq!(r1y.first().unwrap().date, d(2024, 2, 29));

    let r5y = filter_by_range(&snapshots, HistoryRange::FiveYears, d(2025, 2, 28));
    assert_eq!(r5y.first().unwrap().date, d(2023, 2, 28));

    // Month-end & leap-day clamping via chrono::Months
    let leap_snaps = vec![
        make_snapshot(d(2023, 2, 27)),
        make_snapshot(d(2023, 2, 28)),
        make_snapshot(d(2024, 2, 28)),
        make_snapshot(d(2024, 2, 29)),
    ];
    let march31_1m = filter_by_range(&leap_snaps, HistoryRange::OneMonth, d(2024, 3, 31));
    assert_eq!(march31_1m[0].date, d(2024, 2, 29));

    let leap_1y = filter_by_range(&leap_snaps, HistoryRange::OneYear, d(2024, 2, 29));
    assert_eq!(leap_1y[0].date, d(2023, 2, 28));
}

#[test]
fn compute_allocation_deterministic_tie_breaker_and_zero_total() {
    let mut totals = BTreeMap::new();
    totals.insert("etf".to_string(), dec("500"));
    totals.insert("crypto".to_string(), dec("500"));
    totals.insert("cash".to_string(), dec("1000"));

    let alloc = compute_allocation(&totals, dec("2000"));
    assert_eq!(alloc[0].category, "cash");
    assert_eq!(alloc[1].category, "crypto");
    assert_eq!(alloc[2].category, "etf");

    assert_eq!(compute_allocation(&totals, Decimal::ZERO), []);
}

#[test]
fn show_report_case_insensitive_filter_delta_and_skipped_assets() {
    let portfolio = Portfolio {
        version: 1,
        assets: vec![
            Asset {
                id: "vti".to_string(),
                name: "VTI".to_string(),
                category: "etf".to_string(),
                currency: "USD".to_string(),
                archived: false,
            },
            Asset {
                id: "bank".to_string(),
                name: "Ameriabank".to_string(),
                category: "bank".to_string(),
                currency: "AMD".to_string(),
                archived: false,
            },
        ],
        snapshots: vec![
            Snapshot {
                date: d(2025, 1, 1),
                rates: make_rates(&[("AMD", "400")]),
                entries: vec![
                    SnapshotEntry {
                        asset_id: "vti".to_string(),
                        value: dec("10000"),
                    },
                    SnapshotEntry {
                        asset_id: "bank".to_string(),
                        value: dec("400000"),
                    },
                ],
            },
            Snapshot {
                date: d(2025, 2, 1),
                rates: make_rates(&[("AMD", "400")]),
                entries: vec![
                    SnapshotEntry {
                        asset_id: "vti".to_string(),
                        value: dec("12000"),
                    },
                    SnapshotEntry {
                        asset_id: "bank".to_string(),
                        value: dec("400000"),
                    },
                    SnapshotEntry {
                        asset_id: "ghost".to_string(),
                        value: dec("999"),
                    },
                ],
            },
        ],
    };

    let report =
        compute_show_report(&portfolio.snapshots[1], &portfolio, true, Some("ETF")).unwrap();
    assert_eq!(report.grand_total_usd, dec("12000"));
    assert_eq!(report.sections.len(), 1);
    assert_eq!(report.allocation, []);
    assert_eq!(report.skipped_asset_ids, vec!["ghost"]);
    let delta = report.delta_from_previous.unwrap();
    assert_eq!(delta.previous_date, d(2025, 1, 1));
    assert_eq!(delta.change_usd, dec("2000"));
    assert_eq!(delta.change_pct, Some(dec("20")));
}

#[test]
fn history_report_computes_rows_zero_base_and_summary() {
    let portfolio = Portfolio {
        version: 1,
        assets: vec![Asset {
            id: "vti".to_string(),
            name: "VTI".to_string(),
            category: "etf".to_string(),
            currency: "USD".to_string(),
            archived: false,
        }],
        snapshots: vec![
            Snapshot {
                date: d(2025, 1, 1),
                rates: BTreeMap::new(),
                entries: vec![SnapshotEntry {
                    asset_id: "vti".to_string(),
                    value: Decimal::ZERO,
                }],
            },
            Snapshot {
                date: d(2025, 2, 1),
                rates: BTreeMap::new(),
                entries: vec![SnapshotEntry {
                    asset_id: "vti".to_string(),
                    value: dec("100"),
                }],
            },
            Snapshot {
                date: d(2025, 3, 1),
                rates: BTreeMap::new(),
                entries: vec![SnapshotEntry {
                    asset_id: "vti".to_string(),
                    value: dec("150"),
                }],
            },
        ],
    };

    let refs: Vec<&Snapshot> = portfolio.snapshots.iter().collect();
    let report = compute_history_report(&refs, &portfolio, HistoryRange::All, None).unwrap();
    assert_eq!(report.rows.len(), 3);
    assert_eq!(report.rows[0].change_usd, None);
    assert_eq!(report.rows[1].change_usd, Some(dec("100")));
    assert_eq!(report.rows[1].change_pct, None);
    assert_eq!(report.rows[2].change_usd, Some(dec("50")));
    assert_eq!(report.rows[2].change_pct, Some(dec("50")));

    let summary = report.summary.unwrap();
    assert_eq!(summary.start_date, d(2025, 1, 1));
    assert_eq!(summary.end_date, d(2025, 3, 1));
    assert_eq!(summary.change_usd, dec("150"));
    assert_eq!(summary.change_pct, None);
}
