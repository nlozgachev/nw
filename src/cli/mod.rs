use crate::model::{HistoryRange, OutputFormat};
use chrono::NaiveDate;
use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

/// Strictly parse an ISO 8601 `YYYY-MM-DD` date string (zero-padded).
pub fn parse_iso_date(raw: &str) -> Result<NaiveDate, String> {
    if raw.len() != 10 {
        return Err(format!("invalid date '{raw}': expected YYYY-MM-DD"));
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|err| format!("invalid date '{raw}': expected YYYY-MM-DD ({err})"))
}

#[derive(Debug, Parser)]
#[command(name = "nw", version, about = "Net worth tracker CLI")]
pub struct Cli {
    /// Path to portfolio JSON file (overrides default XDG path)
    #[arg(short = 'f', long, global = true, env = "NW_FILE", value_name = "PATH")]
    pub file: Option<PathBuf>,

    /// Disable colored terminal output
    #[arg(long, global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage assets
    Asset(AssetArgs),
    /// Manage snapshots
    #[command(alias = "snap")]
    Snapshot(SnapshotArgs),
    /// Show current (or past) net worth
    Show(ShowArgs),
    /// Show net worth history over a time range
    History(HistoryArgs),
    /// Print the active portfolio file path
    Path,
    /// Validate the portfolio file and report summary statistics
    Validate,
    /// Generate shell completion scripts
    Completions(CompletionsArgs),
}

#[derive(Debug, Args)]
pub struct AssetArgs {
    #[command(subcommand)]
    pub subcommand: AssetSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum AssetSubcommand {
    /// Add a new asset
    Add(AssetAddArgs),
    /// Edit an existing asset
    Edit(AssetEditArgs),
    /// Remove an asset
    #[command(alias = "rm")]
    Remove(AssetRemoveArgs),
    /// Archive an asset (hide from future snapshots while keeping history)
    Archive(AssetIdArg),
    /// Unarchive a previously archived asset
    Unarchive(AssetIdArg),
    /// List assets
    #[command(alias = "ls")]
    List(AssetListArgs),
}

#[derive(Debug, Args)]
pub struct AssetAddArgs {
    #[arg(short, long)]
    pub id: String,
    #[arg(short, long)]
    pub name: String,
    #[arg(short, long)]
    pub category: String,
    #[arg(long)]
    pub currency: String,
}

#[derive(Debug, Args)]
pub struct AssetEditArgs {
    #[arg(short, long)]
    pub id: String,
    #[arg(short, long)]
    pub name: Option<String>,
    #[arg(short, long)]
    pub category: Option<String>,
    #[arg(long)]
    pub currency: Option<String>,
    /// Allow changing currency even when snapshots already reference this asset
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct AssetRemoveArgs {
    #[arg(short, long)]
    pub id: String,
    /// Also delete historical entries for this asset from all snapshots
    #[arg(long)]
    pub purge: bool,
    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct AssetIdArg {
    #[arg(short, long)]
    pub id: String,
}

#[derive(Debug, Args)]
pub struct AssetListArgs {
    /// Include archived assets
    #[arg(short, long)]
    pub all: bool,
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct SnapshotArgs {
    #[command(subcommand)]
    pub subcommand: SnapshotSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SnapshotSubcommand {
    /// Add a new snapshot (defaults to today's date)
    Add(SnapshotAddArgs),
    /// Edit an existing snapshot
    Edit(SnapshotEditArgs),
    /// Remove an existing snapshot
    #[command(alias = "rm")]
    Remove(SnapshotRemoveArgs),
    /// List all snapshots
    #[command(alias = "ls")]
    List(SnapshotListArgs),
}

#[derive(Debug, Args)]
pub struct SnapshotAddArgs {
    /// Snapshot date in YYYY-MM-DD format (default: today)
    #[arg(short, long, value_parser = parse_iso_date)]
    pub date: Option<NaiveDate>,
    /// Allow pressing Enter to carry forward values and FX rates from the previous snapshot
    #[arg(long)]
    pub carry_forward: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotEditArgs {
    /// Snapshot date in YYYY-MM-DD format
    #[arg(short, long, value_parser = parse_iso_date)]
    pub date: NaiveDate,
    /// Skip overwrite confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotRemoveArgs {
    /// Snapshot date in YYYY-MM-DD format
    #[arg(short, long, value_parser = parse_iso_date)]
    pub date: NaiveDate,
    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct SnapshotListArgs {
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Show snapshot for a specific date (default: latest)
    #[arg(short, long, value_parser = parse_iso_date)]
    pub date: Option<NaiveDate>,
    /// Filter display to one category (case-insensitive)
    #[arg(short, long)]
    pub category: Option<String>,
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    /// Time range: 1M, 6M, 1Y, 5Y, ALL (default: 1Y)
    #[arg(short, long, value_enum, ignore_case = true, default_value_t = HistoryRange::OneYear)]
    pub range: HistoryRange,
    /// Filter history to one category (case-insensitive)
    #[arg(short, long)]
    pub category: Option<String>,
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// Target shell
    #[arg(value_enum)]
    pub shell: Shell,
}

#[cfg(test)]
mod tests;
