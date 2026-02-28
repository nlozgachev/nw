use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use crate::error::NwError;
use crate::model::Asset;

/// Prompt for exchange rates for each non-USD currency.
/// `existing_rates` pre-fills values when editing (shown in brackets).
pub fn prompt_rates(
    currencies: &[String],
    existing_rates: Option<&HashMap<String, f64>>,
) -> Result<HashMap<String, f64>, NwError> {
    let mut rates = HashMap::new();

    if currencies.is_empty() {
        return Ok(rates);
    }

    println!("--- Exchange Rates ---");
    for currency in currencies {
        if currency == "USD" {
            return Err(NwError::UsdRateRejected);
        }

        let existing = existing_rates.and_then(|r| r.get(currency));
        let prompt = match existing {
            Some(v) => format!("{} rate (1 USD = ? {}) [{}]: ", currency, currency, v),
            None => format!("{} rate (1 USD = ? {}): ", currency, currency),
        };

        loop {
            let input = read_line(&prompt)?;
            let trimmed = input.trim();

            if trimmed.is_empty() {
                if let Some(v) = existing {
                    rates.insert(currency.clone(), *v);
                    break;
                } else {
                    println!("  Rate is required.");
                    continue;
                }
            }

            match trimmed.parse::<f64>() {
                Ok(v) if v > 0.0 => {
                    rates.insert(currency.clone(), v);
                    break;
                }
                Ok(_) => println!("  Rate must be a positive number."),
                Err(_) => println!("  Invalid number. Please try again."),
            }
        }
    }

    Ok(rates)
}

/// Prompt for asset values. Press Enter to omit an asset.
/// `existing_entries` pre-fills values when editing.
pub fn prompt_asset_values(
    assets: &[Asset],
    existing_entries: Option<&HashMap<String, f64>>,
) -> Result<Vec<(String, f64)>, NwError> {
    let mut entries = Vec::new();

    if assets.is_empty() {
        return Ok(entries);
    }

    println!("--- Asset Values (press Enter to omit) ---");
    for asset in assets {
        let existing = existing_entries.and_then(|m| m.get(&asset.id));
        let prompt = match existing {
            Some(v) => format!(
                "{} ({}, {}) [{}]: ",
                asset.name,
                asset.category.to_uppercase(),
                asset.currency,
                v
            ),
            None => format!(
                "{} ({}, {}): ",
                asset.name,
                asset.category.to_uppercase(),
                asset.currency
            ),
        };

        loop {
            let input = read_line(&prompt)?;
            let trimmed = input.trim();

            if trimmed.is_empty() {
                if let Some(v) = existing {
                    entries.push((asset.id.clone(), *v));
                }
                // no existing → omit asset
                break;
            }

            match trimmed.parse::<f64>() {
                Ok(v) if v >= 0.0 => {
                    entries.push((asset.id.clone(), v));
                    break;
                }
                Ok(_) => println!("  Value must be non-negative."),
                Err(_) => println!("  Invalid number. Please try again."),
            }
        }
    }

    Ok(entries)
}

/// Ask a yes/no confirmation question. Defaults to No.
pub fn confirm(message: &str) -> bool {
    let input = read_line(message).unwrap_or_default();
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn read_line(prompt: &str) -> Result<String, NwError> {
    print!("{}", prompt);
    io::stdout().flush().map_err(|e| NwError::WriteFile {
        path: "stdout".to_string(),
        source: e,
    })?;
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).map_err(|e| NwError::ReadFile {
        path: "stdin".to_string(),
        source: e,
    })?;
    Ok(line)
}