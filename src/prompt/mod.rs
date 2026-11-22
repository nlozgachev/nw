use crate::error::NwError;
use crate::model::{Asset, BASE_CURRENCY, Snapshot, SnapshotEntry};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as FmtWrite;
use std::io::{BufRead, Write};
use std::str::FromStr;

/// Parse user-entered numbers, stripping grouping commas and underscores.
pub fn parse_numeric_input(raw: &str) -> Option<Decimal> {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|&c| c != ',' && c != '_')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    Decimal::from_str(&cleaned).ok()
}

/// Read one line after writing `prompt`. Returns `NwError::Aborted` on EOF.
pub fn read_line<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
) -> Result<String, NwError> {
    write!(writer, "{prompt}")?;
    writer.flush()?;
    let mut line = String::new();
    let bytes = reader.read_line(&mut line)?;
    if bytes == 0 {
        return Err(NwError::Aborted);
    }
    Ok(line)
}

/// Prompt for exchange rates for each non-USD currency.
pub fn prompt_rates<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    currencies: &[String],
    existing_rates: Option<&BTreeMap<String, Decimal>>,
    previous_hints: Option<&BTreeMap<String, Decimal>>,
    carry_forward: bool,
) -> Result<BTreeMap<String, Decimal>, NwError> {
    let mut rates = BTreeMap::new();

    if currencies.is_empty() {
        return Ok(rates);
    }

    writeln!(writer, "--- Exchange Rates ---")?;
    for currency in currencies {
        if currency.eq_ignore_ascii_case(BASE_CURRENCY) {
            return Err(NwError::UsdRateRejected);
        }

        let existing = existing_rates.and_then(|r| r.get(currency)).copied();
        let prev = previous_hints.and_then(|r| r.get(currency)).copied();

        let prompt = if let Some(v) = existing {
            format!("{currency} rate (1 USD = ? {currency}) [{v}]: ")
        } else if let Some(v) = prev {
            if carry_forward {
                format!("{currency} rate (1 USD = ? {currency}) [{v}]: ")
            } else {
                format!("{currency} rate (1 USD = ? {currency}) [prev: {v}]: ")
            }
        } else {
            format!("{currency} rate (1 USD = ? {currency}): ")
        };

        let fallback = existing.or(if carry_forward { prev } else { None });

        loop {
            let input = read_line(reader, writer, &prompt)?;
            let trimmed = input.trim();

            if trimmed.is_empty() {
                if let Some(v) = fallback {
                    rates.insert(currency.clone(), v);
                    break;
                }
                writeln!(writer, "  Rate is required.")?;
                continue;
            }

            match parse_numeric_input(trimmed) {
                Some(v) if v > Decimal::ZERO => {
                    rates.insert(currency.clone(), v);
                    break;
                }
                Some(_) => writeln!(writer, "  Rate must be a positive number.")?,
                None => writeln!(writer, "  Invalid number. Please try again.")?,
            }
        }
    }

    Ok(rates)
}

/// Prompt for asset values. Supports omitting via Enter or clearing via `-` / `clear`.
pub fn prompt_asset_values<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    assets: &[&Asset],
    existing_entries: Option<&BTreeMap<String, Decimal>>,
    previous_hints: Option<&BTreeMap<String, Decimal>>,
    carry_forward: bool,
) -> Result<Vec<SnapshotEntry>, NwError> {
    let mut entries = Vec::new();

    if assets.is_empty() {
        return Ok(entries);
    }

    if existing_entries.is_some() {
        writeln!(
            writer,
            "--- Asset Values (press Enter to keep existing, '-' to omit) ---"
        )?;
    } else if carry_forward && previous_hints.is_some() {
        writeln!(
            writer,
            "--- Asset Values (press Enter to keep previous, '-' to omit) ---"
        )?;
    } else {
        writeln!(writer, "--- Asset Values (press Enter to omit) ---")?;
    }

    for asset in assets {
        let existing = existing_entries.and_then(|m| m.get(&asset.id)).copied();
        let prev = previous_hints.and_then(|m| m.get(&asset.id)).copied();
        let cat_upper = asset.category.to_uppercase();

        let prompt = if let Some(v) = existing {
            format!("{} ({cat_upper}, {}) [{v}]: ", asset.name, asset.currency)
        } else if let Some(v) = prev {
            if carry_forward {
                format!("{} ({cat_upper}, {}) [{v}]: ", asset.name, asset.currency)
            } else {
                format!(
                    "{} ({cat_upper}, {}) [prev: {v}]: ",
                    asset.name, asset.currency
                )
            }
        } else {
            format!("{} ({cat_upper}, {}): ", asset.name, asset.currency)
        };

        let fallback = existing.or(if carry_forward { prev } else { None });

        loop {
            let input = read_line(reader, writer, &prompt)?;
            let trimmed = input.trim();

            if trimmed == "-" || trimmed.eq_ignore_ascii_case("clear") {
                break;
            }

            if trimmed.is_empty() {
                if let Some(v) = fallback {
                    entries.push(SnapshotEntry {
                        asset_id: asset.id.clone(),
                        value: v,
                    });
                }
                break;
            }

            match parse_numeric_input(trimmed) {
                Some(v) if v >= Decimal::ZERO => {
                    entries.push(SnapshotEntry {
                        asset_id: asset.id.clone(),
                        value: v,
                    });
                    break;
                }
                Some(_) => writeln!(writer, "  Value must be non-negative.")?,
                None => writeln!(writer, "  Invalid number. Please try again.")?,
            }
        }
    }

    Ok(entries)
}

/// Ask a yes/no confirmation question. Defaults to No; returns `NwError::Aborted` on EOF.
pub fn confirm<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    message: &str,
) -> Result<bool, NwError> {
    let prompt = format!("{message} ");
    let input = read_line(reader, writer, &prompt)?;
    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// Format a human-readable diff between an existing snapshot and proposed changes.
pub fn format_snapshot_diff(
    old: &Snapshot,
    new_rates: &BTreeMap<String, Decimal>,
    new_entries: &[SnapshotEntry],
    assets: &[Asset],
) -> String {
    let mut out = String::new();

    let all_currencies: BTreeSet<&str> = old
        .rates
        .keys()
        .chain(new_rates.keys())
        .map(String::as_str)
        .collect();

    for cur in all_currencies {
        match (old.rates.get(cur), new_rates.get(cur)) {
            (Some(o), Some(n)) if o != n => {
                let _ = writeln!(out, "  rate {cur}: {o} -> {n}");
            }
            (None, Some(n)) => {
                let _ = writeln!(out, "  rate {cur}: (none) -> {n}");
            }
            (Some(o), None) => {
                let _ = writeln!(out, "  rate {cur}: {o} -> (removed)");
            }
            _ => {}
        }
    }

    let name_map: HashMap<&str, &str> = assets
        .iter()
        .map(|a| (a.id.as_str(), a.name.as_str()))
        .collect();
    let old_map: HashMap<&str, Decimal> = old
        .entries
        .iter()
        .map(|e| (e.asset_id.as_str(), e.value))
        .collect();
    let new_map: HashMap<&str, Decimal> = new_entries
        .iter()
        .map(|e| (e.asset_id.as_str(), e.value))
        .collect();

    let all_ids: BTreeSet<&str> = old_map.keys().chain(new_map.keys()).copied().collect();
    for id in all_ids {
        let label = name_map.get(id).copied().unwrap_or(id);
        match (old_map.get(id), new_map.get(id)) {
            (Some(o), Some(n)) if o != n => {
                let _ = writeln!(out, "  {label}: {o} -> {n}");
            }
            (None, Some(n)) => {
                let _ = writeln!(out, "  {label}: (omitted) -> {n}");
            }
            (Some(o), None) => {
                let _ = writeln!(out, "  {label}: {o} -> (omitted)");
            }
            _ => {}
        }
    }

    if out.is_empty() {
        out.push_str("  (no changes)\n");
    }
    out
}

#[cfg(test)]
mod tests;
