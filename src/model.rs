use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use crate::error::NwError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Portfolio {
    pub assets: Vec<Asset>,
    pub snapshots: Vec<Snapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub category: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub date: String,
    pub rates: HashMap<String, f64>,
    pub entries: Vec<SnapshotEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub asset_id: String,
    pub value: f64,
}

// View models — never serialized

pub struct ShowRow {
    pub asset_name: String,
    pub currency: String,
    pub native_value: f64,
    pub usd_value: f64,
    pub category: String,
}

pub struct HistoryRow {
    pub date: String,
    pub total_usd: f64,
    pub change_usd: Option<f64>,
    pub change_pct: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRange {
    OneMonth,
    SixMonths,
    OneYear,
    FiveYears,
    All,
}

impl FromStr for HistoryRange {
    type Err = NwError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "1M" => Ok(HistoryRange::OneMonth),
            "6M" => Ok(HistoryRange::SixMonths),
            "1Y" => Ok(HistoryRange::OneYear),
            "5Y" => Ok(HistoryRange::FiveYears),
            "ALL" => Ok(HistoryRange::All),
            _ => Err(NwError::InvalidHistoryRange(s.to_string())),
        }
    }
}

impl std::fmt::Display for HistoryRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistoryRange::OneMonth => write!(f, "1M"),
            HistoryRange::SixMonths => write!(f, "6M"),
            HistoryRange::OneYear => write!(f, "1Y"),
            HistoryRange::FiveYears => write!(f, "5Y"),
            HistoryRange::All => write!(f, "ALL"),
        }
    }
}
