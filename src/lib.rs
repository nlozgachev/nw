pub mod cli;
pub mod compute;
pub mod display;
pub mod error;
pub mod model;
pub mod prompt;
pub mod store;

use chrono::NaiveDate;
use clap::CommandFactory;
use cli::{AssetSubcommand, Cli, Command, SnapshotSubcommand};
use display::DisplayOptions;
use error::NwError;
use model::{OutputFormat, Portfolio, Snapshot};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use store::PortfolioStore;

/// Execute a parsed `Cli` command using injected I/O streams, current date, and TTY flag.
pub fn run_with_io<R: BufRead, W: Write, E: Write>(
    cli: Cli,
    reader: &mut R,
    stdout: &mut W,
    stderr: &mut E,
    today: NaiveDate,
    stdout_is_tty: bool,
) -> Result<(), NwError> {
    let color_enabled =
        stdout_is_tty && !cli.no_color && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty());

    match cli.command {
        Command::Path => {
            let store = PortfolioStore::from_override(cli.file)?;
            writeln!(stdout, "{}", store.path().display())?;
            Ok(())
        }
        Command::Completions(args) => {
            let mut cmd = Cli::command();
            clap_complete::generate(args.shell, &mut cmd, "nw", stdout);
            Ok(())
        }
        Command::Validate => {
            let store = PortfolioStore::from_override(cli.file)?;
            let portfolio = store.load()?;
            writeln!(
                stdout,
                "Valid portfolio at {}: {} asset(s), {} snapshot(s).",
                store.path().display(),
                portfolio.assets.len(),
                portfolio.snapshots.len()
            )?;
            Ok(())
        }
        Command::Asset(args) => {
            let store = PortfolioStore::from_override(cli.file)?;
            let mut portfolio = store.load()?;
            handle_asset(args, &mut portfolio, &store, reader, stdout, color_enabled)
        }
        Command::Snapshot(args) => {
            let store = PortfolioStore::from_override(cli.file)?;
            let mut portfolio = store.load()?;
            handle_snapshot(
                args,
                &mut portfolio,
                &store,
                reader,
                stdout,
                today,
                color_enabled,
            )
        }
        Command::Show(args) => {
            let store = PortfolioStore::from_override(cli.file)?;
            let portfolio = store.load()?;
            handle_show(&args, &portfolio, stdout, stderr, color_enabled)
        }
        Command::History(args) => {
            let store = PortfolioStore::from_override(cli.file)?;
            let portfolio = store.load()?;
            handle_history(&args, &portfolio, stdout, stderr, today, color_enabled)
        }
    }
}

fn handle_asset<R: BufRead, W: Write>(
    args: cli::AssetArgs,
    portfolio: &mut Portfolio,
    store: &PortfolioStore,
    reader: &mut R,
    stdout: &mut W,
    color_enabled: bool,
) -> Result<(), NwError> {
    match args.subcommand {
        AssetSubcommand::Add(a) => {
            portfolio.add_asset(&a.id, &a.name, &a.category, &a.currency)?;
            store.save(portfolio)?;
            writeln!(stdout, "Asset added.")?;
        }
        AssetSubcommand::Edit(a) => {
            let changed = portfolio.edit_asset(
                &a.id,
                a.name.as_deref(),
                a.category.as_deref(),
                a.currency.as_deref(),
                a.force,
            )?;
            if changed {
                store.save(portfolio)?;
                writeln!(stdout, "Asset updated.")?;
            } else {
                writeln!(stdout, "Nothing to update.")?;
            }
        }
        AssetSubcommand::Remove(a) => {
            if !portfolio.assets.iter().any(|x| x.id == a.id) {
                return Err(NwError::AssetNotFound(a.id));
            }
            let count = portfolio.snapshot_count_for_asset(&a.id);
            if count > 0 && !a.yes {
                let action_note = if a.purge {
                    "and purge its historical entries"
                } else {
                    "(tip: use 'nw asset archive' to preserve historical totals cleanly)"
                };
                let msg = format!(
                    "This asset appears in {count} snapshot(s) {action_note}. Remove anyway? (y/N)"
                );
                if !prompt::confirm(reader, stdout, &msg)? {
                    writeln!(stdout, "Aborted.")?;
                    return Ok(());
                }
            }
            portfolio.remove_asset(&a.id, a.purge)?;
            store.save(portfolio)?;
            writeln!(stdout, "Asset removed.")?;
        }
        AssetSubcommand::Archive(a) => {
            portfolio.set_asset_archived(&a.id, true)?;
            store.save(portfolio)?;
            writeln!(stdout, "Asset '{}' archived.", a.id)?;
        }
        AssetSubcommand::Unarchive(a) => {
            portfolio.set_asset_archived(&a.id, false)?;
            store.save(portfolio)?;
            writeln!(stdout, "Asset '{}' unarchived.", a.id)?;
        }
        AssetSubcommand::List(a) => {
            let assets: Vec<&model::Asset> = if a.all {
                portfolio.assets.iter().collect()
            } else {
                portfolio.active_assets()
            };
            let opts = DisplayOptions {
                format: a.format,
                color: color_enabled && a.format == OutputFormat::Table,
            };
            display::render_asset_list(&assets, opts, stdout)?;
        }
    }
    Ok(())
}

fn handle_snapshot<R: BufRead, W: Write>(
    args: cli::SnapshotArgs,
    portfolio: &mut Portfolio,
    store: &PortfolioStore,
    reader: &mut R,
    stdout: &mut W,
    today: NaiveDate,
    color_enabled: bool,
) -> Result<(), NwError> {
    match args.subcommand {
        SnapshotSubcommand::Add(a) => {
            handle_snapshot_add(&a, portfolio, store, reader, stdout, today)
        }
        SnapshotSubcommand::Edit(a) => handle_snapshot_edit(&a, portfolio, store, reader, stdout),
        SnapshotSubcommand::Remove(a) => {
            handle_snapshot_remove(&a, portfolio, store, reader, stdout)
        }
        SnapshotSubcommand::List(a) => {
            let opts = DisplayOptions {
                format: a.format,
                color: color_enabled && a.format == OutputFormat::Table,
            };
            display::render_snapshot_list(&portfolio.snapshots, opts, stdout)
        }
    }
}

fn handle_snapshot_add<R: BufRead, W: Write>(
    args: &cli::SnapshotAddArgs,
    portfolio: &mut Portfolio,
    store: &PortfolioStore,
    reader: &mut R,
    stdout: &mut W,
    today: NaiveDate,
) -> Result<(), NwError> {
    let date = args.date.unwrap_or(today);
    if portfolio.find_snapshot(date).is_some() {
        return Err(NwError::SnapshotAlreadyExists(date));
    }
    let active_assets = portfolio.active_assets();
    if active_assets.is_empty() {
        return Err(NwError::NoActiveAssets);
    }

    let prev_snapshot = portfolio.previous_snapshot(date);
    let prev_rates = prev_snapshot.map(|s| &s.rates);
    let prev_entries: Option<BTreeMap<String, Decimal>> = prev_snapshot.map(|s| {
        s.entries
            .iter()
            .map(|e| (e.asset_id.clone(), e.value))
            .collect()
    });

    let currencies = portfolio.active_non_usd_currencies();
    let rates = prompt::prompt_rates(
        reader,
        stdout,
        &currencies,
        None,
        prev_rates,
        args.carry_forward,
    )?;
    let entries = prompt::prompt_asset_values(
        reader,
        stdout,
        &active_assets,
        None,
        prev_entries.as_ref(),
        args.carry_forward,
    )?;

    portfolio.add_snapshot(Snapshot {
        date,
        rates,
        entries,
    })?;
    store.save(portfolio)?;
    writeln!(stdout, "Snapshot saved for {date}.")?;
    Ok(())
}

fn handle_snapshot_edit<R: BufRead, W: Write>(
    args: &cli::SnapshotEditArgs,
    portfolio: &mut Portfolio,
    store: &PortfolioStore,
    reader: &mut R,
    stdout: &mut W,
) -> Result<(), NwError> {
    let existing = portfolio
        .find_snapshot(args.date)
        .cloned()
        .ok_or(NwError::SnapshotNotFound(args.date))?;

    let existing_entries: BTreeMap<String, Decimal> = existing
        .entries
        .iter()
        .map(|e| (e.asset_id.clone(), e.value))
        .collect();

    // Prompt across active assets plus any archived asset already present in this snapshot.
    let prompt_assets: Vec<&model::Asset> = portfolio
        .assets
        .iter()
        .filter(|asset| !asset.archived || existing_entries.contains_key(&asset.id))
        .collect();

    let mut currencies: Vec<String> = prompt_assets
        .iter()
        .filter(|asset| asset.currency != model::BASE_CURRENCY)
        .map(|asset| asset.currency.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if currencies.is_empty() && !existing.rates.is_empty() {
        currencies = existing.rates.keys().cloned().collect();
    }

    let rates = prompt::prompt_rates(
        reader,
        stdout,
        &currencies,
        Some(&existing.rates),
        None,
        false,
    )?;
    let entries = prompt::prompt_asset_values(
        reader,
        stdout,
        &prompt_assets,
        Some(&existing_entries),
        None,
        false,
    )?;

    if !args.yes {
        let diff = prompt::format_snapshot_diff(&existing, &rates, &entries, &portfolio.assets);
        writeln!(stdout, "\nChanges for {}:\n{diff}", args.date)?;
        if !prompt::confirm(
            reader,
            stdout,
            &format!("Overwrite snapshot for {}? (y/N)", args.date),
        )? {
            writeln!(stdout, "Aborted.")?;
            return Ok(());
        }
    }

    portfolio.replace_snapshot(args.date, rates, entries)?;
    store.save(portfolio)?;
    writeln!(stdout, "Snapshot updated.")?;
    Ok(())
}

fn handle_snapshot_remove<R: BufRead, W: Write>(
    args: &cli::SnapshotRemoveArgs,
    portfolio: &mut Portfolio,
    store: &PortfolioStore,
    reader: &mut R,
    stdout: &mut W,
) -> Result<(), NwError> {
    if portfolio.find_snapshot(args.date).is_none() {
        return Err(NwError::SnapshotNotFound(args.date));
    }
    if !args.yes
        && !prompt::confirm(
            reader,
            stdout,
            &format!("Delete snapshot for {}? (y/N)", args.date),
        )?
    {
        writeln!(stdout, "Aborted.")?;
        return Ok(());
    }
    portfolio.remove_snapshot(args.date)?;
    store.save(portfolio)?;
    writeln!(stdout, "Snapshot removed.")?;
    Ok(())
}

fn handle_show<W: Write, E: Write>(
    args: &cli::ShowArgs,
    portfolio: &Portfolio,
    stdout: &mut W,
    stderr: &mut E,
    color_enabled: bool,
) -> Result<(), NwError> {
    let Some(latest) = portfolio.latest_snapshot() else {
        if args.format == OutputFormat::Json {
            writeln!(stdout, "null")?;
        } else {
            writeln!(stdout, "No snapshots yet.")?;
        }
        return Ok(());
    };

    let (snapshot, is_latest) = match args.date {
        Some(date) => {
            let found = portfolio
                .find_snapshot(date)
                .ok_or(NwError::SnapshotNotFound(date))?;
            (found, found.date == latest.date)
        }
        None => (latest, true),
    };

    let report =
        compute::compute_show_report(snapshot, portfolio, is_latest, args.category.as_deref())?;

    if !report.skipped_asset_ids.is_empty() {
        writeln!(
            stderr,
            "warning: snapshot {} references unknown asset id(s): {}",
            snapshot.date,
            report.skipped_asset_ids.join(", ")
        )?;
    }

    let opts = DisplayOptions {
        format: args.format,
        color: color_enabled && args.format == OutputFormat::Table,
    };
    display::render_show(&report, opts, stdout)
}

fn handle_history<W: Write, E: Write>(
    args: &cli::HistoryArgs,
    portfolio: &Portfolio,
    stdout: &mut W,
    stderr: &mut E,
    today: NaiveDate,
    color_enabled: bool,
) -> Result<(), NwError> {
    let filtered = compute::filter_by_range(&portfolio.snapshots, args.range, today);
    if filtered.is_empty() && args.format == OutputFormat::Table {
        writeln!(stdout, "No snapshots in range.")?;
        return Ok(());
    }

    let report = compute::compute_history_report(
        &filtered,
        portfolio,
        args.range,
        args.category.as_deref(),
    )?;

    if !report.skipped_asset_ids.is_empty() {
        writeln!(
            stderr,
            "warning: snapshot(s) reference unknown asset id(s): {}",
            report.skipped_asset_ids.join(", ")
        )?;
    }

    let opts = DisplayOptions {
        format: args.format,
        color: color_enabled && args.format == OutputFormat::Table,
    };
    display::render_history(&report, opts, stdout)
}
