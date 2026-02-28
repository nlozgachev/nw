use thiserror::Error;

#[derive(Debug, Error)]
pub enum NwError {
    #[error("asset id '{0}' already exists")]
    DuplicateAssetId(String),

    #[error("asset id '{0}' not found")]
    AssetNotFound(String),

    #[error("snapshot for date '{0}' already exists")]
    SnapshotAlreadyExists(String),

    #[error("snapshot for date '{0}' not found")]
    SnapshotNotFound(String),

    #[error("USD is the base currency and cannot have a rate")]
    UsdRateRejected,

    // #[error("unknown asset_id '{0}' in snapshot entry")]
    // UnknownAssetInEntry(String),
    #[error("invalid date format '{0}': expected YYYY-MM-DD")]
    InvalidDate(String),

    #[error("invalid history range '{0}': expected 1M, 6M, 1Y, 5Y, or ALL")]
    InvalidHistoryRange(String),

    #[error("failed to read portfolio file at {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to write portfolio file at {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    #[error("malformed JSON in {path}: {source}")]
    MalformedJson {
        path: String,
        source: serde_json::Error,
    },

    #[error("failed to serialize portfolio at {path}: {source}")]
    SerializeJson {
        path: String,
        source: serde_json::Error,
    },

    #[error("could not determine config directory")]
    NoConfigDir,

    #[error("no rate found for currency '{0}'")]
    RateMissing(String),
    // #[error("no snapshots found in portfolio")]
    // NoSnapshots,
}
