use super::*;
use crate::model::{
    AllocationRow, CategorySection, DeltaSummary, HistoryRange, HistoryRow, PeriodSummary, ShowRow,
};
use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::str::FromStr;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

#[test]
fn number_formatting_handles_commas_rounding_neg_zero_and_large_values() {
    assert_eq!(fmt_currency(dec("0")), "0.00");
    assert_eq!(fmt_currency(dec("-0.001")), "0.00");
    assert_eq!(fmt_currency(dec("12500.5")), "12,500.50");
    assert_eq!(fmt_currency(dec("-12500.005")), "-12,500.01");
    assert_eq!(fmt_currency(dec("999.999")), "1,000.00");
    assert_eq!(
        fmt_currency(dec("12345678901234567890123.45")),
        "12,345,678,901,234,567,890,123.45"
    );

    assert_eq!(fmt_change(dec("-0.001")), "0.00");
    assert_eq!(fmt_change(dec("2800")), "+2,800.00");
    assert_eq!(fmt_change(dec("-1300")), "-1,300.00");

    assert_eq!(fmt_pct(dec("-0.001")), "0.00%");
    assert_eq!(fmt_pct(dec("6.624")), "+6.62%");
    assert_eq!(fmt_pct(dec("-2.886")), "-2.89%");
}

#[test]
fn render_show_uses_correct_headers_and_formats() {
    let report = ShowReport {
        date: d(2024, 6, 1),
        is_latest: false,
        category_filter: None,
        sections: vec![CategorySection {
            category: "real-estate-long-name".to_string(),
            rows: vec![ShowRow {
                asset_id: "home".to_string(),
                asset_name: "Apartment".to_string(),
                currency: "USD".to_string(),
                native_value: dec("100000"),
                usd_value: dec("100000"),
                category: "real-estate-long-name".to_string(),
            }],
            subtotal_usd: dec("100000"),
        }],
        grand_total_usd: dec("100000"),
        delta_from_previous: Some(DeltaSummary {
            previous_date: d(2024, 1, 1),
            change_usd: dec("5000"),
            change_pct: Some(dec("5.26")),
        }),
        allocation: vec![AllocationRow {
            category: "real-estate-long-name".to_string(),
            total_usd: dec("100000"),
            pct: dec("100"),
        }],
        skipped_asset_ids: vec![],
    };

    let mut buf = Vec::new();
    render_show(&report, DisplayOptions::default(), &mut buf).unwrap();
    let out = String::from_utf8(buf).unwrap();
    assert!(out.starts_with("NET WORTH — 2024-06-01"));
    assert!(!out.contains("CURRENT NET WORTH"));
    assert!(out.contains("TOTAL  100,000.00  (+5,000.00, +5.26% vs 2024-01-01)"));
    assert!(out.contains("REAL-ESTATE-LONG-NAME  100.0%"));

    let mut json_buf = Vec::new();
    render_show(
        &report,
        DisplayOptions {
            format: OutputFormat::Json,
            color: false,
        },
        &mut json_buf,
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&json_buf).unwrap();
    assert_eq!(parsed["grand_total_usd"], serde_json::json!(100_000));

    let mut csv_buf = Vec::new();
    render_show(
        &report,
        DisplayOptions {
            format: OutputFormat::Csv,
            color: false,
        },
        &mut csv_buf,
    )
    .unwrap();
    let csv_out = String::from_utf8(csv_buf).unwrap();
    assert!(csv_out.contains("2024-06-01,real-estate-long-name,home,Apartment,USD,100000,100000"));
}

#[test]
fn render_history_and_snapshot_list_output() {
    let report = HistoryReport {
        range: HistoryRange::OneYear,
        category_filter: None,
        rows: vec![
            HistoryRow {
                date: d(2025, 1, 1),
                total_usd: dec("1000"),
                change_usd: None,
                change_pct: None,
            },
            HistoryRow {
                date: d(2025, 2, 1),
                total_usd: dec("1250"),
                change_usd: Some(dec("250")),
                change_pct: Some(dec("25")),
            },
        ],
        summary: Some(PeriodSummary {
            start_date: d(2025, 1, 1),
            end_date: d(2025, 2, 1),
            change_usd: dec("250"),
            change_pct: Some(dec("25")),
        }),
        skipped_asset_ids: vec![],
    };

    let mut buf = Vec::new();
    render_history(&report, DisplayOptions::default(), &mut buf).unwrap();
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Period change (2025-01-01 -> 2025-02-01): +250.00 (+25.00%)"));

    let mut rates = BTreeMap::new();
    rates.insert("EUR".to_string(), dec("0.92"));
    let snapshots = vec![
        Snapshot {
            date: d(2025, 1, 1),
            rates: BTreeMap::new(),
            entries: vec![],
        },
        Snapshot {
            date: d(2025, 2, 1),
            rates,
            entries: vec![],
        },
    ];

    let mut snap_buf = Vec::new();
    render_snapshot_list(&snapshots, DisplayOptions::default(), &mut snap_buf).unwrap();
    let snap_out = String::from_utf8(snap_buf).unwrap();
    assert!(snap_out.contains("EUR=0.92"));
    assert!(snap_out.contains('—'));
    assert!(!snap_out.contains("USD only"));
}
