mod cli;
mod compute;
mod display;
mod error;
mod model;
mod prompt;
mod store;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, AssetSubcommand, SnapshotSubcommand};
use model::HistoryRange;
use std::str::FromStr;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut portfolio = store::load_portfolio()?;

    match cli.command {
        Command::Asset(args) => handle_asset(args, &mut portfolio)?,
        Command::Snapshot(args) => handle_snapshot(args, &mut portfolio)?,
        Command::Show(args) => handle_show(args, &portfolio)?,
        Command::History(args) => handle_history(args, &portfolio)?,
    }

    Ok(())
}

fn handle_asset(args: cli::AssetArgs, portfolio: &mut model::Portfolio) -> Result<()> {
    match args.subcommand {
        AssetSubcommand::Add(a) => {
            let currency = a.currency.to_uppercase();
            if portfolio.assets.iter().any(|x| x.id == a.id) {
                return Err(error::NwError::DuplicateAssetId(a.id).into());
            }
            portfolio.assets.push(model::Asset {
                id: a.id,
                name: a.name,
                category: a.category.to_lowercase(),
                currency,
            });
            store::save_portfolio(portfolio)?;
            println!("Asset added.");
        }
        AssetSubcommand::Edit(a) => {
            let asset = portfolio
                .assets
                .iter_mut()
                .find(|x| x.id == a.id)
                .ok_or_else(|| error::NwError::AssetNotFound(a.id.clone()))?;
            let mut changed = false;
            if let Some(name) = a.name { asset.name = name; changed = true; }
            if let Some(cat) = a.category { asset.category = cat.to_lowercase(); changed = true; }
            if let Some(cur) = a.currency { asset.currency = cur.to_uppercase(); changed = true; }
            if changed {
                store::save_portfolio(portfolio)?;
                println!("Asset updated.");
            } else {
                println!("Nothing to update.");
            }
        }
        AssetSubcommand::Remove(a) => {
            if !portfolio.assets.iter().any(|x| x.id == a.id) {
                return Err(error::NwError::AssetNotFound(a.id).into());
            }
            let count = portfolio
                .snapshots
                .iter()
                .filter(|s| s.entries.iter().any(|e| e.asset_id == a.id))
                .count();
            if !prompt::confirm(&format!(
                "This asset appears in {count} snapshot(s). Are you sure? (y/N)"
            )) {
                println!("Aborted.");
                return Ok(());
            }
            portfolio.assets.retain(|x| x.id != a.id);
            store::save_portfolio(portfolio)?;
            println!("Asset removed.");
        }
        AssetSubcommand::List => {
            display::print_asset_list(&portfolio.assets);
        }
    }
    Ok(())
}

fn handle_snapshot(args: cli::SnapshotArgs, portfolio: &mut model::Portfolio) -> Result<()> {
    match args.subcommand {
        SnapshotSubcommand::Add(a) => {
            validate_date(&a.date)?;
            if portfolio.snapshots.iter().any(|s| s.date == a.date) {
                return Err(error::NwError::SnapshotAlreadyExists(a.date).into());
            }
            let currencies = collect_non_usd_currencies(portfolio);
            let rates = prompt::prompt_rates(&currencies, None)?;
            let entries_raw = prompt::prompt_asset_values(&portfolio.assets, None)?;
            let entries = entries_raw
                .into_iter()
                .map(|(id, val)| model::SnapshotEntry { asset_id: id, value: val })
                .collect();
            portfolio.snapshots.push(model::Snapshot {
                date: a.date,
                rates,
                entries,
            });
            store::save_portfolio(portfolio)?;
            println!("Snapshot saved.");
        }
        SnapshotSubcommand::Edit(a) => {
            validate_date(&a.date)?;
            let idx = portfolio
                .snapshots
                .iter()
                .position(|s| s.date == a.date)
                .ok_or_else(|| error::NwError::SnapshotNotFound(a.date.clone()))?;
            if !prompt::confirm(&format!("Overwrite snapshot for {}? (y/N)", a.date)) {
                println!("Aborted.");
                return Ok(());
            }
            let existing = portfolio.snapshots[idx].clone();
            let currencies = collect_non_usd_currencies(portfolio);
            let rates = prompt::prompt_rates(&currencies, Some(&existing.rates))?;
            let existing_map: std::collections::HashMap<String, f64> = existing
                .entries
                .iter()
                .map(|e| (e.asset_id.clone(), e.value))
                .collect();
            let entries_raw =
                prompt::prompt_asset_values(&portfolio.assets, Some(&existing_map))?;
            let entries = entries_raw
                .into_iter()
                .map(|(id, val)| model::SnapshotEntry { asset_id: id, value: val })
                .collect();
            portfolio.snapshots[idx].rates = rates;
            portfolio.snapshots[idx].entries = entries;
            store::save_portfolio(portfolio)?;
            println!("Snapshot updated.");
        }
        SnapshotSubcommand::List => {
            display::print_snapshot_list(&portfolio.snapshots);
        }
    }
    Ok(())
}

fn handle_show(args: cli::ShowArgs, portfolio: &model::Portfolio) -> Result<()> {
    if portfolio.snapshots.is_empty() {
        println!("No snapshots yet.");
        return Ok(());
    }

    let snapshot = if let Some(date) = &args.date {
        validate_date(date)?;
        portfolio
            .snapshots
            .iter()
            .find(|s| &s.date == date)
            .ok_or_else(|| error::NwError::SnapshotNotFound(date.clone()))?
    } else {
        portfolio.snapshots.last().expect("non-empty checked above")
    };

    let category_filter = args.category.as_deref();
    let (grand_total, rows) =
        compute::compute_show_rows(snapshot, portfolio, category_filter)?;

    let mut category_totals = std::collections::HashMap::new();
    for row in &rows {
        *category_totals.entry(row.category.clone()).or_insert(0.0) += row.usd_value;
    }
    let allocation = compute::compute_allocation(&category_totals, grand_total);

    display::print_show(rows, grand_total, allocation, &snapshot.date, category_filter);
    Ok(())
}

fn handle_history(args: cli::HistoryArgs, portfolio: &model::Portfolio) -> Result<()> {
    let range = HistoryRange::from_str(&args.range)?;
    let today = chrono::Local::now().date_naive().to_string();
    let filtered = compute::filter_by_range(&portfolio.snapshots, range, &today);
    if filtered.is_empty() {
        println!("No snapshots in range.");
        return Ok(());
    }
    let history_rows = compute::compute_history_rows(&filtered, portfolio)?;
    display::print_history(history_rows, &range.to_string());
    Ok(())
}

fn validate_date(date: &str) -> Result<()> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| error::NwError::InvalidDate(date.to_string()))?;
    Ok(())
}

fn collect_non_usd_currencies(portfolio: &model::Portfolio) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut currencies = Vec::new();
    for asset in &portfolio.assets {
        if asset.currency != "USD" && seen.insert(asset.currency.clone()) {
            currencies.push(asset.currency.clone());
        }
    }
    currencies.sort();
    currencies
}
