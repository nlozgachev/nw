use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NwError {
    #[error("asset id '{0}' already exists")]
    DuplicateAssetId(String),

    #[error("asset id '{0}' not found")]
    AssetNotFound(String),

    #[error("{field} cannot be empty")]
    EmptyField { field: &'static str },

    #[error("invalid currency code '{0}': expected non-empty alphanumeric code (e.g. USD, EUR)")]
    InvalidCurrency(String),

    #[error("asset '{id}' is already {state}")]
    AssetArchiveState { id: String, state: &'static str },

    #[error(
        "cannot change currency of asset '{id}': it appears in {count} snapshot(s) (pass --force to override)"
    )]
    CurrencyChangeBlocked { id: String, count: usize },

    #[error("snapshot for date '{0}' already exists")]
    SnapshotAlreadyExists(NaiveDate),

    #[error("snapshot for date '{0}' not found")]
    SnapshotNotFound(NaiveDate),

    #[error(
        "cannot create snapshot: no active assets exist (add an asset first with 'nw asset add')"
    )]
    NoActiveAssets,

    #[error("USD is the base currency and cannot have a rate")]
    UsdRateRejected,

    #[error("invalid exchange rate '{rate}' for '{currency}': must be positive")]
    InvalidRate { currency: String, rate: Decimal },

    #[error("invalid value '{value}' for asset '{asset_id}': must be non-negative")]
    InvalidValue { asset_id: String, value: Decimal },

    #[error("duplicate entry for asset '{asset_id}' in snapshot '{date}'")]
    DuplicateSnapshotEntry { date: NaiveDate, asset_id: String },

    #[error("duplicate snapshot for date '{0}'")]
    DuplicateSnapshotDate(NaiveDate),

    #[error("unsupported portfolio schema version {found} (supported: 1..={supported})")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },

    #[error("invalid date '{0}': expected YYYY-MM-DD")]
    InvalidDate(String),

    #[error("invalid history range '{0}': expected 1M, 6M, 1Y, 5Y, or ALL")]
    InvalidHistoryRange(String),

    #[error("no rate found for currency '{0}'")]
    RateMissing(String),

    #[error("failed to read portfolio file at {}: {source}", path.display())]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write portfolio file at {}: {source}", path.display())]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("malformed JSON in {}: {source}", path.display())]
    MalformedJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to serialize portfolio at {}: {source}", path.display())]
    SerializeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("could not determine config directory")]
    NoConfigDir,

    #[error("terminal I/O error: {0}")]
    TerminalIo(#[from] std::io::Error),

    #[error("aborted")]
    Aborted,
}
