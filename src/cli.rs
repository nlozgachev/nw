use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nw", about = "Net worth tracker CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage assets
    Asset(AssetArgs),
    /// Manage snapshots
    Snapshot(SnapshotArgs),
    /// Show current (or past) net worth
    Show(ShowArgs),
    /// Show net worth history over a time range
    History(HistoryArgs),
}

#[derive(Args)]
pub struct AssetArgs {
    #[command(subcommand)]
    pub subcommand: AssetSubcommand,
}

#[derive(Subcommand)]
pub enum AssetSubcommand {
    /// Add a new asset
    Add(AssetAddArgs),
    /// Edit an existing asset
    Edit(AssetEditArgs),
    /// Remove an asset
    Remove(AssetRemoveArgs),
    /// List all assets
    List,
}

#[derive(Args)]
pub struct AssetAddArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub category: String,
    #[arg(long)]
    pub currency: String,
}

#[derive(Args)]
pub struct AssetEditArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub currency: Option<String>,
}

#[derive(Args)]
pub struct AssetRemoveArgs {
    #[arg(long)]
    pub id: String,
}

#[derive(Args)]
pub struct SnapshotArgs {
    #[command(subcommand)]
    pub subcommand: SnapshotSubcommand,
}

#[derive(Subcommand)]
pub enum SnapshotSubcommand {
    /// Add a new snapshot
    Add(SnapshotDateArg),
    /// Edit an existing snapshot
    Edit(SnapshotDateArg),
    /// List all snapshots
    List,
}

#[derive(Args)]
pub struct SnapshotDateArg {
    #[arg(long)]
    pub date: String,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Show snapshot for a specific date (default: latest)
    #[arg(long)]
    pub date: Option<String>,
    /// Filter display to one category
    #[arg(long)]
    pub category: Option<String>,
}

#[derive(Args)]
pub struct HistoryArgs {
    /// Time range: 1M, 6M, 1Y, 5Y, ALL
    #[arg(long)]
    pub range: String,
}
