use std::collections::HashMap;
use chrono::{Datelike, NaiveDate};
use crate::error::NwError;
use crate::model::{HistoryRange, HistoryRow, Portfolio, ShowRow, Snapshot};

/// Convert a value in `currency` to USD using the snapshot's rate map.
/// USD assets return `value` unchanged.
/// Rates are stored as "1 USD = N foreign units", so: value_usd = native_value / rate.
pub fn to_usd(value: f64, currency: &str, rates: &HashMap<String, f64>) -> Result<f64, NwError> {
    if currency == "USD" {
        return Ok(value);
    }
    rates
        .get(currency)
        .map(|rate| value / rate)
        .ok_or_else(|| NwError::RateMissing(currency.to_string()))
}

/// Compute ShowRows from a snapshot. Unknown asset_ids in entries are silently skipped.
/// Returns (grand_total_usd, Vec<ShowRow>) where grand_total accounts for the category filter.
pub fn compute_show_rows(
    snapshot: &Snapshot,
    portfolio: &Portfolio,
    category_filter: Option<&str>,
) -> Result<(f64, Vec<ShowRow>), NwError> {
    let asset_map: HashMap<&str, &crate::model::Asset> =
        portfolio.assets.iter().map(|a| (a.id.as_str(), a)).collect();

    let mut rows = Vec::new();
    let mut grand_total = 0.0;

    for entry in &snapshot.entries {
        let asset = match asset_map.get(entry.asset_id.as_str()) {
            Some(a) => a,
            None => continue, // silently skip removed assets
        };

        if let Some(filter) = category_filter {
            if asset.category != filter {
                continue;
            }
        }

        let usd_value = to_usd(entry.value, &asset.currency, &snapshot.rates)?;
        grand_total += usd_value;
        rows.push(ShowRow {
            asset_name: asset.name.clone(),
            currency: asset.currency.clone(),
            native_value: entry.value,
            usd_value,
            category: asset.category.clone(),
        });
    }

    Ok((grand_total, rows))
}

/// Compute allocation percentages. Returns Vec<(category, pct)> sorted by pct descending.
pub fn compute_allocation(
    category_totals: &HashMap<String, f64>,
    grand_total: f64,
) -> Vec<(String, f64)> {
    if grand_total == 0.0 {
        return Vec::new();
    }
    let mut result: Vec<(String, f64)> = category_totals
        .iter()
        .map(|(cat, total)| (cat.clone(), total / grand_total * 100.0))
        .collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Filter snapshots to those within the given range, anchored at `today` (YYYY-MM-DD).
pub fn filter_by_range<'a>(
    snapshots: &'a [Snapshot],
    range: HistoryRange,
    today: &str,
) -> Vec<&'a Snapshot> {
    if range == HistoryRange::All {
        return snapshots.iter().collect();
    }

    let today_date = match NaiveDate::parse_from_str(today, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return snapshots.iter().collect(),
    };

    let cutoff = match range {
        HistoryRange::OneMonth => subtract_months(today_date, 1),
        HistoryRange::SixMonths => subtract_months(today_date, 6),
        HistoryRange::OneYear => subtract_years(today_date, 1),
        HistoryRange::FiveYears => subtract_years(today_date, 5),
        HistoryRange::All => unreachable!(),
    };

    let cutoff_str = cutoff.format("%Y-%m-%d").to_string();

    snapshots
        .iter()
        .filter(|s| s.date.as_str() >= cutoff_str.as_str())
        .collect()
}

fn subtract_months(date: NaiveDate, months: u32) -> NaiveDate {
    let mut year = date.year();
    let mut month = date.month() as i32 - months as i32;
    while month <= 0 {
        month += 12;
        year -= 1;
    }
    let day = date.day().min(days_in_month(year, month as u32));
    NaiveDate::from_ymd_opt(year, month as u32, day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, month as u32, 1).expect("valid year/month"))
}

fn subtract_years(date: NaiveDate, years: i32) -> NaiveDate {
    let year = date.year() - years;
    let day = date.day().min(days_in_month(year, date.month()));
    NaiveDate::from_ymd_opt(year, date.month(), day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, date.month(), 1).expect("valid year/month"))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Compute total USD value of all entries in a snapshot (skipping unknown asset_ids).
pub fn snapshot_total_usd(snapshot: &Snapshot, portfolio: &Portfolio) -> Result<f64, NwError> {
    let (total, _) = compute_show_rows(snapshot, portfolio, None)?;
    Ok(total)
}

/// Build HistoryRow list. First row has change = None.
pub fn compute_history_rows(
    snapshots: &[&Snapshot],
    portfolio: &Portfolio,
) -> Result<Vec<HistoryRow>, NwError> {
    let mut rows = Vec::new();
    let mut prev_total: Option<f64> = None;

    for snapshot in snapshots {
        let total_usd = snapshot_total_usd(snapshot, portfolio)?;
        let (change_usd, change_pct) = match prev_total {
            Some(prev) => {
                let (cu, cp) = compute_change(prev, total_usd);
                (Some(cu), Some(cp))
            }
            None => (None, None),
        };
        rows.push(HistoryRow {
            date: snapshot.date.clone(),
            total_usd,
            change_usd,
            change_pct,
        });
        prev_total = Some(total_usd);
    }

    Ok(rows)
}

/// Returns (change_usd, change_pct). If prev == 0, change_pct is 0.0.
pub fn compute_change(prev: f64, current: f64) -> (f64, f64) {
    let change_usd = current - prev;
    let change_pct = if prev == 0.0 { 0.0 } else { (change_usd / prev) * 100.0 };
    (change_usd, change_pct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Asset, Portfolio, Snapshot, SnapshotEntry};

    fn make_rates(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    fn make_snapshot(date: &str) -> Snapshot {
        Snapshot { date: date.to_string(), rates: HashMap::new(), entries: vec![] }
    }

    // ---- to_usd ----

    #[test]
    fn test_to_usd_passthrough() {
        let rates = HashMap::new();
        assert_eq!(to_usd(1000.0, "USD", &rates).unwrap(), 1000.0);
    }

    #[test]
    fn test_to_usd_foreign() {
        // 1 USD = 0.92 EUR, so 800 EUR / 0.92 ≈ 869.57 USD
        let rates = make_rates(&[("EUR", 0.92)]);
        let result = to_usd(800.0, "EUR", &rates).unwrap();
        assert!((result - 869.6).abs() < 0.1);
    }

    #[test]
    fn test_to_usd_missing_rate() {
        let rates = HashMap::new();
        assert!(to_usd(100.0, "EUR", &rates).is_err());
    }

    // ---- compute_change ----

    #[test]
    fn test_compute_change_positive() {
        let (change, pct) = compute_change(42300.0, 45100.0);
        assert!((change - 2800.0).abs() < 0.01);
        assert!((pct - 6.62).abs() < 0.01);
    }

    #[test]
    fn test_compute_change_negative() {
        let (change, pct) = compute_change(45100.0, 43800.0);
        assert!((change - (-1300.0)).abs() < 0.01);
        assert!((pct - (-2.88)).abs() < 0.01);
    }

    #[test]
    fn test_compute_change_from_zero() {
        let (change, pct) = compute_change(0.0, 100.0);
        assert!((change - 100.0).abs() < 0.01);
        assert_eq!(pct, 0.0);
    }

    // ---- filter_by_range ----

    #[test]
    fn test_filter_all() {
        let snapshots = vec![make_snapshot("2020-01-01"), make_snapshot("2025-02-28")];
        let result = filter_by_range(&snapshots, HistoryRange::All, "2025-02-28");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_1y() {
        let snapshots = vec![
            make_snapshot("2023-12-01"),
            make_snapshot("2024-03-01"),
            make_snapshot("2025-02-28"),
        ];
        // today = 2025-02-28, 1Y cutoff = 2024-02-28
        let result = filter_by_range(&snapshots, HistoryRange::OneYear, "2025-02-28");
        // 2023-12-01 is before cutoff, others are after
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].date, "2024-03-01");
        assert_eq!(result[1].date, "2025-02-28");
    }

    #[test]
    fn test_filter_1m() {
        let snapshots = vec![
            make_snapshot("2025-01-15"),
            make_snapshot("2025-02-10"),
            make_snapshot("2025-02-28"),
        ];
        // today = 2025-02-28, 1M cutoff = 2025-01-28
        let result = filter_by_range(&snapshots, HistoryRange::OneMonth, "2025-02-28");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].date, "2025-02-10");
    }

    #[test]
    fn test_filter_6m() {
        let snapshots = vec![
            make_snapshot("2024-07-01"),
            make_snapshot("2024-09-01"),
            make_snapshot("2025-02-28"),
        ];
        // today = 2025-02-28, 6M cutoff = 2024-08-28
        let result = filter_by_range(&snapshots, HistoryRange::SixMonths, "2025-02-28");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].date, "2024-09-01");
    }

    #[test]
    fn test_filter_5y() {
        let snapshots = vec![
            make_snapshot("2019-12-31"),
            make_snapshot("2020-03-01"),
            make_snapshot("2025-02-28"),
        ];
        // today = 2025-02-28, 5Y cutoff = 2020-02-28
        let result = filter_by_range(&snapshots, HistoryRange::FiveYears, "2025-02-28");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].date, "2020-03-01");
    }

    // ---- snapshot sort order ----

    #[test]
    fn test_snapshot_sort_ascending() {
        let mut snapshots = vec![
            make_snapshot("2025-06-01"),
            make_snapshot("2024-01-01"),
            make_snapshot("2025-01-15"),
        ];
        snapshots.sort_by(|a, b| a.date.cmp(&b.date));
        assert_eq!(snapshots[0].date, "2024-01-01");
        assert_eq!(snapshots[1].date, "2025-01-15");
        assert_eq!(snapshots[2].date, "2025-06-01");
    }

    // ---- compute_allocation ----

    #[test]
    fn test_compute_allocation() {
        let mut totals = HashMap::new();
        totals.insert("etf".to_string(), 1670.0);
        totals.insert("crypto".to_string(), 320.0);
        totals.insert("bank".to_string(), 646.0);
        let alloc = compute_allocation(&totals, 2636.0);
        // Should be sorted descending by pct
        assert_eq!(alloc[0].0, "etf");
        assert!(alloc[0].1 > alloc[1].1);
    }

    #[test]
    fn test_compute_allocation_zero_total() {
        let totals = HashMap::new();
        let alloc = compute_allocation(&totals, 0.0);
        assert!(alloc.is_empty());
    }

    // ---- compute_show_rows ----

    #[test]
    fn test_compute_show_rows_usd_asset() {
        let portfolio = Portfolio {
            assets: vec![Asset {
                id: "vti".to_string(),
                name: "VTI".to_string(),
                category: "etf".to_string(),
                currency: "USD".to_string(),
            }],
            snapshots: vec![],
        };
        let snapshot = Snapshot {
            date: "2025-01-01".to_string(),
            rates: HashMap::new(),
            entries: vec![SnapshotEntry { asset_id: "vti".to_string(), value: 12500.0 }],
        };
        let (total, rows) = compute_show_rows(&snapshot, &portfolio, None).unwrap();
        assert!((total - 12500.0).abs() < 0.01);
        assert_eq!(rows.len(), 1);
        assert!((rows[0].usd_value - 12500.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_show_rows_foreign_asset() {
        let portfolio = Portfolio {
            assets: vec![Asset {
                id: "amd-bank".to_string(),
                name: "Ameriabank".to_string(),
                category: "bank".to_string(),
                currency: "AMD".to_string(),
            }],
            snapshots: vec![],
        };
        let snapshot = Snapshot {
            date: "2025-01-01".to_string(),
            rates: make_rates(&[("AMD", 387.5)]),
            entries: vec![SnapshotEntry {
                asset_id: "amd-bank".to_string(),
                value: 2_500_000.0,
            }],
        };
        let (total, rows) = compute_show_rows(&snapshot, &portfolio, None).unwrap();
        // 2,500,000 AMD / 387.5 = ~6451.6 USD
        assert!((total - 6451.6).abs() < 1.0);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_compute_show_rows_skips_unknown_asset() {
        let portfolio = Portfolio { assets: vec![], snapshots: vec![] };
        let snapshot = Snapshot {
            date: "2025-01-01".to_string(),
            rates: HashMap::new(),
            entries: vec![SnapshotEntry { asset_id: "ghost".to_string(), value: 100.0 }],
        };
        let (total, rows) = compute_show_rows(&snapshot, &portfolio, None).unwrap();
        assert_eq!(total, 0.0);
        assert!(rows.is_empty());
    }

    #[test]
    fn test_compute_show_rows_category_filter() {
        let portfolio = Portfolio {
            assets: vec![
                Asset {
                    id: "vti".to_string(),
                    name: "VTI".to_string(),
                    category: "etf".to_string(),
                    currency: "USD".to_string(),
                },
                Asset {
                    id: "btc".to_string(),
                    name: "Bitcoin".to_string(),
                    category: "crypto".to_string(),
                    currency: "USD".to_string(),
                },
            ],
            snapshots: vec![],
        };
        let snapshot = Snapshot {
            date: "2025-01-01".to_string(),
            rates: HashMap::new(),
            entries: vec![
                SnapshotEntry { asset_id: "vti".to_string(), value: 12500.0 },
                SnapshotEntry { asset_id: "btc".to_string(), value: 3200.0 },
            ],
        };
        let (total, rows) = compute_show_rows(&snapshot, &portfolio, Some("etf")).unwrap();
        assert!((total - 12500.0).abs() < 0.01);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].asset_name, "VTI");
    }
}
