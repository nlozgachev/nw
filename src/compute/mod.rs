use crate::error::NwError;
use crate::model::{
    AllocationRow, Asset, BASE_CURRENCY, CategorySection, DeltaSummary, HistoryRange,
    HistoryReport, HistoryRow, PeriodSummary, Portfolio, ShowReport, ShowRow, Snapshot,
};
use chrono::{Months, NaiveDate};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Convert a value in `currency` to USD using the snapshot's rate map.
/// USD assets return `value` unchanged.
/// Rates are stored as "1 USD = N foreign units", so: `value_usd = native_value / rate`.
pub fn to_usd(
    value: Decimal,
    currency: &str,
    rates: &BTreeMap<String, Decimal>,
) -> Result<Decimal, NwError> {
    if currency.eq_ignore_ascii_case(BASE_CURRENCY) {
        return Ok(value);
    }
    let rate = rates
        .get(currency)
        .copied()
        .ok_or_else(|| NwError::RateMissing(currency.to_string()))?;
    if rate <= Decimal::ZERO {
        return Err(NwError::InvalidRate {
            currency: currency.to_string(),
            rate,
        });
    }
    Ok(value / rate)
}

/// Compute `(change_usd, change_pct)`. `change_pct` is `None` when `prev` is zero.
pub fn compute_change(prev: Decimal, current: Decimal) -> (Decimal, Option<Decimal>) {
    let change_usd = current - prev;
    let change_pct = if prev.is_zero() {
        None
    } else {
        Some((change_usd / prev) * Decimal::ONE_HUNDRED)
    };
    (change_usd, change_pct)
}

/// Compute allocation percentages sorted descending by percentage, then ascending by category.
pub fn compute_allocation(
    category_totals: &BTreeMap<String, Decimal>,
    grand_total: Decimal,
) -> Vec<AllocationRow> {
    if grand_total.is_zero() {
        return Vec::new();
    }

    let mut rows: Vec<AllocationRow> = category_totals
        .iter()
        .map(|(category, &total_usd)| AllocationRow {
            category: category.clone(),
            total_usd,
            pct: (total_usd / grand_total) * Decimal::ONE_HUNDRED,
        })
        .collect();

    rows.sort_by(|a, b| b.pct.cmp(&a.pct).then_with(|| a.category.cmp(&b.category)));
    rows
}

/// Filter snapshots to those within `range` anchored at `today`.
pub fn filter_by_range(
    snapshots: &[Snapshot],
    range: HistoryRange,
    today: NaiveDate,
) -> Vec<&Snapshot> {
    let cutoff = match range {
        HistoryRange::OneMonth => Some(
            today
                .checked_sub_months(Months::new(1))
                .unwrap_or(NaiveDate::MIN),
        ),
        HistoryRange::SixMonths => Some(
            today
                .checked_sub_months(Months::new(6))
                .unwrap_or(NaiveDate::MIN),
        ),
        HistoryRange::OneYear => Some(
            today
                .checked_sub_months(Months::new(12))
                .unwrap_or(NaiveDate::MIN),
        ),
        HistoryRange::FiveYears => Some(
            today
                .checked_sub_months(Months::new(60))
                .unwrap_or(NaiveDate::MIN),
        ),
        HistoryRange::All => None,
    };

    snapshots
        .iter()
        .filter(|s| cutoff.is_none_or(|c| s.date >= c))
        .collect()
}

/// Compute total USD value of matching entries in a snapshot without allocating `ShowRow`s.
/// Unknown `asset_id`s are skipped and returned (sorted/deduplicated).
pub fn snapshot_total_usd<S: std::hash::BuildHasher>(
    snapshot: &Snapshot,
    asset_map: &HashMap<&str, &Asset, S>,
    category_filter: Option<&str>,
) -> Result<(Decimal, Vec<String>), NwError> {
    let filter_lower = category_filter.map(str::to_lowercase);
    let mut total = Decimal::ZERO;
    let mut skipped = BTreeSet::new();

    for entry in &snapshot.entries {
        let Some(&asset) = asset_map.get(entry.asset_id.as_str()) else {
            skipped.insert(entry.asset_id.clone());
            continue;
        };
        if let Some(ref f) = filter_lower
            && asset.category.to_lowercase() != *f
        {
            continue;
        }
        total += to_usd(entry.value, &asset.currency, &snapshot.rates)?;
    }

    Ok((total, skipped.into_iter().collect()))
}

/// Build a full structured `ShowReport` for `snapshot`.
pub fn compute_show_report(
    snapshot: &Snapshot,
    portfolio: &Portfolio,
    is_latest: bool,
    category_filter: Option<&str>,
) -> Result<ShowReport, NwError> {
    let asset_map: HashMap<&str, &Asset> = portfolio
        .assets
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect();
    let filter_lower = category_filter.map(str::to_lowercase);

    let mut by_category: BTreeMap<String, Vec<ShowRow>> = BTreeMap::new();
    let mut category_totals: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut grand_total = Decimal::ZERO;
    let mut skipped = BTreeSet::new();

    for entry in &snapshot.entries {
        let Some(&asset) = asset_map.get(entry.asset_id.as_str()) else {
            skipped.insert(entry.asset_id.clone());
            continue;
        };
        let cat_lower = asset.category.to_lowercase();
        if let Some(ref f) = filter_lower
            && cat_lower != *f
        {
            continue;
        }
        let usd_value = to_usd(entry.value, &asset.currency, &snapshot.rates)?;
        grand_total += usd_value;
        *category_totals
            .entry(cat_lower.clone())
            .or_insert(Decimal::ZERO) += usd_value;
        by_category
            .entry(cat_lower.clone())
            .or_default()
            .push(ShowRow {
                asset_id: asset.id.clone(),
                asset_name: asset.name.clone(),
                currency: asset.currency.clone(),
                native_value: entry.value,
                usd_value,
                category: cat_lower,
            });
    }

    let sections: Vec<CategorySection> = by_category
        .into_iter()
        .map(|(category, rows)| {
            let subtotal_usd = category_totals
                .get(&category)
                .copied()
                .unwrap_or(Decimal::ZERO);
            CategorySection {
                category,
                rows,
                subtotal_usd,
            }
        })
        .collect();

    let allocation = if filter_lower.is_none() {
        compute_allocation(&category_totals, grand_total)
    } else {
        Vec::new()
    };

    let delta_from_previous = if let Some(prev) = portfolio.previous_snapshot(snapshot.date) {
        let (prev_total, prev_skipped) =
            snapshot_total_usd(prev, &asset_map, filter_lower.as_deref())?;
        skipped.extend(prev_skipped);
        let (change_usd, change_pct) = compute_change(prev_total, grand_total);
        Some(DeltaSummary {
            previous_date: prev.date,
            change_usd,
            change_pct,
        })
    } else {
        None
    };

    Ok(ShowReport {
        date: snapshot.date,
        is_latest,
        category_filter: filter_lower,
        sections,
        grand_total_usd: grand_total,
        delta_from_previous,
        allocation,
        skipped_asset_ids: skipped.into_iter().collect(),
    })
}

/// Build a full structured `HistoryReport` from pre-filtered snapshots.
pub fn compute_history_report(
    snapshots: &[&Snapshot],
    portfolio: &Portfolio,
    range: HistoryRange,
    category_filter: Option<&str>,
) -> Result<HistoryReport, NwError> {
    let asset_map: HashMap<&str, &Asset> = portfolio
        .assets
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect();
    let filter_lower = category_filter.map(str::to_lowercase);

    let mut totals: Vec<(NaiveDate, Decimal)> = Vec::with_capacity(snapshots.len());
    let mut skipped = BTreeSet::new();

    for &snapshot in snapshots {
        let (total, s_skipped) = snapshot_total_usd(snapshot, &asset_map, filter_lower.as_deref())?;
        skipped.extend(s_skipped);
        totals.push((snapshot.date, total));
    }

    let mut rows = Vec::with_capacity(totals.len());
    let mut prev_total: Option<Decimal> = None;

    for (date, total_usd) in totals {
        let (change_usd, change_pct) = if let Some(prev) = prev_total {
            let (cu, cp) = compute_change(prev, total_usd);
            (Some(cu), cp)
        } else {
            (None, None)
        };
        prev_total = Some(total_usd);
        rows.push(HistoryRow {
            date,
            total_usd,
            change_usd,
            change_pct,
        });
    }

    let summary = if let (Some(first), Some(last)) = (rows.first(), rows.last())
        && rows.len() >= 2
    {
        let (change_usd, change_pct) = compute_change(first.total_usd, last.total_usd);
        Some(PeriodSummary {
            start_date: first.date,
            end_date: last.date,
            change_usd,
            change_pct,
        })
    } else {
        None
    };

    Ok(HistoryReport {
        range,
        category_filter: filter_lower,
        rows,
        summary,
        skipped_asset_ids: skipped.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests;
