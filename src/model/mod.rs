use crate::error::NwError;
use chrono::NaiveDate;
use clap::ValueEnum;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::str::FromStr;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const BASE_CURRENCY: &str = "USD";

const fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(value: &bool) -> bool {
    !*value
}

mod serde_decimal_map {
    use rust_decimal::Decimal;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    struct ArbDecimal(Decimal);

    impl Serialize for ArbDecimal {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            rust_decimal::serde::arbitrary_precision::serialize(&self.0, serializer)
        }
    }

    impl<'de> Deserialize<'de> for ArbDecimal {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            rust_decimal::serde::arbitrary_precision::deserialize(deserializer).map(Self)
        }
    }

    pub(super) fn serialize<S: Serializer>(
        map: &BTreeMap<String, Decimal>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let wrapped: BTreeMap<&String, ArbDecimal> =
            map.iter().map(|(k, v)| (k, ArbDecimal(*v))).collect();
        wrapped.serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<String, Decimal>, D::Error> {
        let wrapped: BTreeMap<String, ArbDecimal> = BTreeMap::deserialize(deserializer)?;
        Ok(wrapped.into_iter().map(|(k, v)| (k, v.0)).collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Portfolio {
    #[serde(default = "default_schema_version")]
    pub version: u32,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
}

impl Default for Portfolio {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            assets: Vec::new(),
            snapshots: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub category: String,
    pub currency: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub date: NaiveDate,
    #[serde(default, with = "serde_decimal_map")]
    pub rates: BTreeMap<String, Decimal>,
    #[serde(default)]
    pub entries: Vec<SnapshotEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotEntry {
    pub asset_id: String,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub value: Decimal,
}

impl Portfolio {
    /// Normalize and validate a non-empty free-form text field.
    pub fn normalize_non_empty(value: &str, field: &'static str) -> Result<String, NwError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(NwError::EmptyField { field });
        }
        Ok(trimmed.to_string())
    }

    /// Normalize and validate an uppercase currency code.
    pub fn normalize_currency(value: &str) -> Result<String, NwError> {
        let trimmed = value.trim().to_ascii_uppercase();
        if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(NwError::InvalidCurrency(value.to_string()));
        }
        Ok(trimmed)
    }

    /// Validate all portfolio invariants (schema version, unique IDs/dates, valid rates/values).
    pub fn validate(&self) -> Result<(), NwError> {
        if self.version == 0 || self.version > CURRENT_SCHEMA_VERSION {
            return Err(NwError::UnsupportedSchemaVersion {
                found: self.version,
                supported: CURRENT_SCHEMA_VERSION,
            });
        }

        let mut seen_asset_ids = HashSet::with_capacity(self.assets.len());
        for asset in &self.assets {
            if asset.id.trim().is_empty() {
                return Err(NwError::EmptyField { field: "asset id" });
            }
            if asset.name.trim().is_empty() {
                return Err(NwError::EmptyField {
                    field: "asset name",
                });
            }
            if asset.category.trim().is_empty() {
                return Err(NwError::EmptyField {
                    field: "asset category",
                });
            }
            Self::normalize_currency(&asset.currency)?;
            if !seen_asset_ids.insert(asset.id.as_str()) {
                return Err(NwError::DuplicateAssetId(asset.id.clone()));
            }
        }

        let mut seen_dates = HashSet::with_capacity(self.snapshots.len());
        for snapshot in &self.snapshots {
            if !seen_dates.insert(snapshot.date) {
                return Err(NwError::DuplicateSnapshotDate(snapshot.date));
            }
            validate_snapshot_contents(snapshot.date, &snapshot.rates, &snapshot.entries)?;
        }

        Ok(())
    }

    /// Sort snapshots chronologically ascending.
    pub fn sort_snapshots(&mut self) {
        self.snapshots.sort_by_key(|s| s.date);
    }

    /// Add a new asset, enforcing unique IDs and normalized fields.
    pub fn add_asset(
        &mut self,
        id: &str,
        name: &str,
        category: &str,
        currency: &str,
    ) -> Result<&Asset, NwError> {
        let id = Self::normalize_non_empty(id, "asset id")?;
        let name = Self::normalize_non_empty(name, "asset name")?;
        let category = Self::normalize_non_empty(category, "asset category")?.to_lowercase();
        let currency = Self::normalize_currency(currency)?;

        if self.assets.iter().any(|a| a.id == id) {
            return Err(NwError::DuplicateAssetId(id));
        }

        self.assets.push(Asset {
            id,
            name,
            category,
            currency,
            archived: false,
        });
        self.assets
            .last()
            .ok_or_else(|| NwError::AssetNotFound(String::new()))
    }

    /// Edit an existing asset. Returns `Ok(true)` if any field changed.
    pub fn edit_asset(
        &mut self,
        id: &str,
        name: Option<&str>,
        category: Option<&str>,
        currency: Option<&str>,
        force_currency_change: bool,
    ) -> Result<bool, NwError> {
        let normalized_name = name
            .map(|v| Self::normalize_non_empty(v, "asset name"))
            .transpose()?;
        let normalized_category = category
            .map(|v| Self::normalize_non_empty(v, "asset category").map(|s| s.to_lowercase()))
            .transpose()?;
        let normalized_currency = currency.map(Self::normalize_currency).transpose()?;

        let snapshot_count = self.snapshot_count_for_asset(id);
        let asset = self
            .assets
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| NwError::AssetNotFound(id.to_string()))?;

        if let Some(ref new_currency) = normalized_currency
            && new_currency != &asset.currency
            && snapshot_count > 0
            && !force_currency_change
        {
            return Err(NwError::CurrencyChangeBlocked {
                id: id.to_string(),
                count: snapshot_count,
            });
        }

        let changed = update_if_changed(&mut asset.name, normalized_name)
            | update_if_changed(&mut asset.category, normalized_category)
            | update_if_changed(&mut asset.currency, normalized_currency);

        Ok(changed)
    }

    /// Archive or unarchive an asset.
    pub fn set_asset_archived(&mut self, id: &str, archived: bool) -> Result<(), NwError> {
        let asset = self
            .assets
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| NwError::AssetNotFound(id.to_string()))?;

        if asset.archived == archived {
            let state = if archived { "archived" } else { "active" };
            return Err(NwError::AssetArchiveState {
                id: id.to_string(),
                state,
            });
        }

        asset.archived = archived;
        Ok(())
    }

    /// Count how many snapshots include an entry for `id`.
    pub fn snapshot_count_for_asset(&self, id: &str) -> usize {
        self.snapshots
            .iter()
            .filter(|s| s.entries.iter().any(|e| e.asset_id == id))
            .count()
    }

    /// Remove an asset and optionally purge its entries from all snapshots.
    /// Returns the number of snapshots that contained this asset.
    pub fn remove_asset(&mut self, id: &str, purge_entries: bool) -> Result<usize, NwError> {
        let pos = self
            .assets
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| NwError::AssetNotFound(id.to_string()))?;

        let count = self.snapshot_count_for_asset(id);
        self.assets.remove(pos);

        if purge_entries && count > 0 {
            for snapshot in &mut self.snapshots {
                snapshot.entries.retain(|e| e.asset_id != id);
            }
        }

        Ok(count)
    }

    /// Return all non-archived assets in insertion order.
    pub fn active_assets(&self) -> Vec<&Asset> {
        self.assets.iter().filter(|a| !a.archived).collect()
    }

    /// Collect sorted distinct non-USD currencies across active assets.
    pub fn active_non_usd_currencies(&self) -> Vec<String> {
        self.assets
            .iter()
            .filter(|a| !a.archived && a.currency != BASE_CURRENCY)
            .map(|a| a.currency.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Add a new snapshot, ensuring unique date, valid contents, and sorted order.
    pub fn add_snapshot(&mut self, snapshot: Snapshot) -> Result<(), NwError> {
        if self.snapshots.iter().any(|s| s.date == snapshot.date) {
            return Err(NwError::SnapshotAlreadyExists(snapshot.date));
        }
        validate_snapshot_contents(snapshot.date, &snapshot.rates, &snapshot.entries)?;
        self.snapshots.push(snapshot);
        self.sort_snapshots();
        Ok(())
    }

    /// Replace rates and entries for an existing snapshot date.
    pub fn replace_snapshot(
        &mut self,
        date: NaiveDate,
        rates: BTreeMap<String, Decimal>,
        entries: Vec<SnapshotEntry>,
    ) -> Result<(), NwError> {
        validate_snapshot_contents(date, &rates, &entries)?;
        let snapshot = self
            .snapshots
            .iter_mut()
            .find(|s| s.date == date)
            .ok_or(NwError::SnapshotNotFound(date))?;
        snapshot.rates = rates;
        snapshot.entries = entries;
        Ok(())
    }

    /// Remove a snapshot by date.
    pub fn remove_snapshot(&mut self, date: NaiveDate) -> Result<Snapshot, NwError> {
        let pos = self
            .snapshots
            .iter()
            .position(|s| s.date == date)
            .ok_or(NwError::SnapshotNotFound(date))?;
        Ok(self.snapshots.remove(pos))
    }

    /// Find a snapshot by exact date.
    pub fn find_snapshot(&self, date: NaiveDate) -> Option<&Snapshot> {
        self.snapshots.iter().find(|s| s.date == date)
    }

    /// Return the latest snapshot (chronologically).
    pub fn latest_snapshot(&self) -> Option<&Snapshot> {
        self.snapshots.iter().max_by_key(|s| s.date)
    }

    /// Return the closest snapshot strictly before `date`.
    pub fn previous_snapshot(&self, date: NaiveDate) -> Option<&Snapshot> {
        self.snapshots
            .iter()
            .filter(|s| s.date < date)
            .max_by_key(|s| s.date)
    }
}

fn update_if_changed(target: &mut String, next: Option<String>) -> bool {
    if let Some(val) = next
        && val != *target
    {
        *target = val;
        return true;
    }
    false
}

fn validate_snapshot_contents(
    date: NaiveDate,
    rates: &BTreeMap<String, Decimal>,
    entries: &[SnapshotEntry],
) -> Result<(), NwError> {
    for (currency, &rate) in rates {
        if currency.eq_ignore_ascii_case(BASE_CURRENCY) {
            return Err(NwError::UsdRateRejected);
        }
        Portfolio::normalize_currency(currency)?;
        if rate <= Decimal::ZERO {
            return Err(NwError::InvalidRate {
                currency: currency.clone(),
                rate,
            });
        }
    }

    let mut seen_entries = HashSet::with_capacity(entries.len());
    for entry in entries {
        if entry.asset_id.trim().is_empty() {
            return Err(NwError::EmptyField { field: "asset_id" });
        }
        if !seen_entries.insert(entry.asset_id.as_str()) {
            return Err(NwError::DuplicateSnapshotEntry {
                date,
                asset_id: entry.asset_id.clone(),
            });
        }
        if entry.value < Decimal::ZERO {
            return Err(NwError::InvalidValue {
                asset_id: entry.asset_id.clone(),
                value: entry.value,
            });
        }
    }

    Ok(())
}

// ---- View models & CLI enums ----

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShowRow {
    pub asset_id: String,
    pub asset_name: String,
    pub currency: String,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub native_value: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub usd_value: Decimal,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CategorySection {
    pub category: String,
    pub rows: Vec<ShowRow>,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub subtotal_usd: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AllocationRow {
    pub category: String,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub total_usd: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub pct: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DeltaSummary {
    pub previous_date: NaiveDate,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub change_usd: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision_option")]
    pub change_pct: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShowReport {
    pub date: NaiveDate,
    pub is_latest: bool,
    pub category_filter: Option<String>,
    pub sections: Vec<CategorySection>,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub grand_total_usd: Decimal,
    pub delta_from_previous: Option<DeltaSummary>,
    pub allocation: Vec<AllocationRow>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped_asset_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HistoryRow {
    pub date: NaiveDate,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub total_usd: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision_option")]
    pub change_usd: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::arbitrary_precision_option")]
    pub change_pct: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PeriodSummary {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub change_usd: Decimal,
    #[serde(with = "rust_decimal::serde::arbitrary_precision_option")]
    pub change_pct: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HistoryReport {
    pub range: HistoryRange,
    pub category_filter: Option<String>,
    pub rows: Vec<HistoryRow>,
    pub summary: Option<PeriodSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped_asset_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize)]
pub enum HistoryRange {
    #[value(name = "1M", alias = "1m")]
    OneMonth,
    #[value(name = "6M", alias = "6m")]
    SixMonths,
    #[default]
    #[value(name = "1Y", alias = "1y")]
    OneYear,
    #[value(name = "5Y", alias = "5y")]
    FiveYears,
    #[value(name = "ALL", alias = "all")]
    All,
}

impl FromStr for HistoryRange {
    type Err = NwError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "1M" => Ok(Self::OneMonth),
            "6M" => Ok(Self::SixMonths),
            "1Y" => Ok(Self::OneYear),
            "5Y" => Ok(Self::FiveYears),
            "ALL" => Ok(Self::All),
            _ => Err(NwError::InvalidHistoryRange(s.to_string())),
        }
    }
}

impl std::fmt::Display for HistoryRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OneMonth => write!(f, "1M"),
            Self::SixMonths => write!(f, "6M"),
            Self::OneYear => write!(f, "1Y"),
            Self::FiveYears => write!(f, "5Y"),
            Self::All => write!(f, "ALL"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Csv,
}

#[cfg(test)]
mod tests;
