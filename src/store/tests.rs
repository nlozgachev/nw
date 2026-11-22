use super::*;
use crate::model::{Asset, Snapshot, SnapshotEntry};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;
use tempfile::TempDir;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

#[test]
fn resolve_portfolio_path_handles_empty_xdg_and_fallbacks() {
    let p = resolve_portfolio_path(
        Some(OsStr::new("")),
        Some(OsStr::new("/home/alice")),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        p,
        PathBuf::from("/home/alice/.config/nw-tracker/portfolio.json")
    );

    let p_xdg = resolve_portfolio_path(
        Some(OsStr::new("/custom/cfg")),
        Some(OsStr::new("/home/alice")),
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        p_xdg,
        PathBuf::from("/custom/cfg/nw-tracker/portfolio.json")
    );

    assert!(matches!(
        resolve_portfolio_path(None, None, None, None),
        Err(NwError::NoConfigDir)
    ));
}

#[test]
fn load_missing_file_returns_empty_portfolio_without_creating_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("portfolio.json");
    let store = PortfolioStore::new(path.clone());
    let portfolio = store.load().unwrap();
    assert_eq!(portfolio.assets, []);
    assert_eq!(portfolio.snapshots, []);
    assert!(!path.exists());
}

#[test]
fn save_sorts_snapshots_deterministically_with_trailing_newline() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nested").join("portfolio.json");
    let store = PortfolioStore::new(path.clone());

    let mut rates = BTreeMap::new();
    rates.insert("ZAR".to_string(), dec("18.1"));
    rates.insert("AMD".to_string(), dec("387.5"));
    rates.insert("EUR".to_string(), dec("0.92"));

    let mut portfolio = Portfolio::default();
    portfolio.assets.push(Asset {
        id: "vti".to_string(),
        name: "VTI".to_string(),
        category: "etf".to_string(),
        currency: "USD".to_string(),
        archived: false,
    });
    portfolio.snapshots.push(Snapshot {
        date: d(2025, 6, 1),
        rates: rates.clone(),
        entries: vec![SnapshotEntry {
            asset_id: "vti".to_string(),
            value: dec("200"),
        }],
    });
    portfolio.snapshots.push(Snapshot {
        date: d(2024, 1, 1),
        rates,
        entries: vec![SnapshotEntry {
            asset_id: "vti".to_string(),
            value: dec("100"),
        }],
    });

    store.save(&mut portfolio).unwrap();
    assert_eq!(portfolio.snapshots[0].date, d(2024, 1, 1));
    assert_eq!(portfolio.snapshots[1].date, d(2025, 6, 1));

    let raw = fs::read_to_string(&path).unwrap();
    assert!(raw.ends_with('\n'));
    let amd_pos = raw.find("\"AMD\"").unwrap();
    let eur_pos = raw.find("\"EUR\"").unwrap();
    let zar_pos = raw.find("\"ZAR\"").unwrap();
    assert!(amd_pos < eur_pos && eur_pos < zar_pos);

    let reloaded = store.load().unwrap();
    assert_eq!(reloaded, portfolio);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn load_sorts_unsorted_hand_edited_json_and_rejects_malformed() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("portfolio.json");
    let store = PortfolioStore::new(path.clone());

    fs::write(
        &path,
        r#"{
          "assets": [],
          "snapshots": [
            { "date": "2025-05-01", "rates": {}, "entries": [] },
            { "date": "2024-01-01", "rates": {}, "entries": [] }
          ]
        }"#,
    )
    .unwrap();

    let loaded = store.load().unwrap();
    assert_eq!(loaded.snapshots[0].date, d(2024, 1, 1));
    assert_eq!(loaded.snapshots[1].date, d(2025, 5, 1));

    fs::write(&path, "{ invalid json").unwrap();
    let err = store.load().unwrap_err();
    assert!(matches!(err, NwError::MalformedJson { .. }));
    assert!(err.to_string().contains(&path.display().to_string()));
}
