use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn nw_cmd(file: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nw"));
    cmd.arg("--file").arg(file);
    cmd
}

fn portfolio_file(dir: &TempDir) -> PathBuf {
    dir.path().join("portfolio.json")
}

#[test]
fn version_and_path_and_completions_work() {
    let dir = TempDir::new().unwrap();
    let file = portfolio_file(&dir);

    Command::new(env!("CARGO_BIN_EXE_nw"))
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("nw "));

    nw_cmd(&file)
        .arg("path")
        .assert()
        .success()
        .stdout(predicate::str::contains(file.to_string_lossy().as_ref()));

    Command::new(env!("CARGO_BIN_EXE_nw"))
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef nw"));
}

#[test]
fn asset_lifecycle_currency_protection_archive_and_remove() {
    let dir = TempDir::new().unwrap();
    let file = portfolio_file(&dir);

    // Add USD and EUR assets
    nw_cmd(&file)
        .args([
            "asset",
            "add",
            "-i",
            "vti",
            "-n",
            "VTI",
            "-c",
            "ETF",
            "--currency",
            "usd",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Asset added."));

    nw_cmd(&file)
        .args([
            "asset",
            "add",
            "-i",
            "sav",
            "-n",
            "EUR Bank",
            "-c",
            "bank",
            "--currency",
            "EUR",
        ])
        .assert()
        .success();

    // Add snapshot with comma-formatted number
    nw_cmd(&file)
        .args(["snapshot", "add", "-d", "2025-01-15"])
        .write_stdin("0.92\n12,500.50\n920\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Snapshot saved for 2025-01-15."));

    // Currency change on referenced asset is blocked without --force
    nw_cmd(&file)
        .args(["asset", "edit", "-i", "sav", "--currency", "USD"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("pass --force"));

    // Currency change works with --force
    nw_cmd(&file)
        .args(["asset", "edit", "-i", "sav", "--currency", "USD", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Asset updated."));

    // Restore EUR so snapshot conversion stays valid
    nw_cmd(&file)
        .args(["asset", "edit", "-i", "sav", "--currency", "EUR", "--force"])
        .assert()
        .success();

    // Archive hides asset from default list and future snapshot add
    nw_cmd(&file)
        .args(["asset", "archive", "-i", "sav"])
        .assert()
        .success()
        .stdout(predicate::str::contains("archived"));

    nw_cmd(&file)
        .args(["asset", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("VTI"))
        .stdout(predicate::str::contains("EUR Bank").not());

    nw_cmd(&file)
        .args(["asset", "list", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("archived"));

    nw_cmd(&file)
        .args(["asset", "unarchive", "-i", "sav"])
        .assert()
        .success();

    // Removing unused asset requires no confirmation
    nw_cmd(&file)
        .args([
            "asset",
            "add",
            "-i",
            "tmp",
            "-n",
            "Temp",
            "-c",
            "cash",
            "--currency",
            "USD",
        ])
        .assert()
        .success();

    nw_cmd(&file)
        .args(["asset", "remove", "-i", "tmp"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Asset removed."));

    // Removing referenced asset with --purge --yes cleans entries
    nw_cmd(&file)
        .args(["asset", "remove", "-i", "sav", "--purge", "--yes"])
        .assert()
        .success();

    let json_text = fs::read_to_string(&file).unwrap();
    assert!(!json_text.contains("\"sav\""));
}

#[test]
fn snapshot_eof_abort_and_edit_clear() {
    let dir = TempDir::new().unwrap();
    let file = portfolio_file(&dir);

    // Empty portfolio rejects snapshot add cleanly
    nw_cmd(&file)
        .args(["snapshot", "add"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no active assets"));

    nw_cmd(&file)
        .args([
            "asset",
            "add",
            "-i",
            "vti",
            "-n",
            "VTI",
            "-c",
            "etf",
            "--currency",
            "USD",
        ])
        .assert()
        .success();
    nw_cmd(&file)
        .args([
            "asset",
            "add",
            "-i",
            "eur",
            "-n",
            "EUR Cash",
            "-c",
            "cash",
            "--currency",
            "EUR",
        ])
        .assert()
        .success();

    // EOF during snapshot add aborts with exit code 130 and does not save
    nw_cmd(&file)
        .args(["snapshot", "add", "-d", "2025-01-01"])
        .write_stdin("")
        .assert()
        .code(130)
        .stderr(predicate::str::contains("Aborted."));

    // Add two snapshots
    nw_cmd(&file)
        .args(["snapshot", "add", "-d", "2025-01-01"])
        .write_stdin("0.92\n10000\n920\n")
        .assert()
        .success();

    nw_cmd(&file)
        .args(["snapshot", "add", "-d", "2025-02-01"])
        .write_stdin("0.92\n12000\n920\n")
        .assert()
        .success();

    // Edit snapshot: change EUR rate to 0.90, keep VTI, clear EUR Cash with `-`, confirm `y`
    nw_cmd(&file)
        .args(["snapshot", "edit", "-d", "2025-02-01"])
        .write_stdin("0.90\n\n-\ny\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("EUR Cash: 920 -> (omitted)"))
        .stdout(predicate::str::contains("Snapshot updated."));
}

#[test]
fn show_history_validate_and_snapshot_remove() {
    let dir = TempDir::new().unwrap();
    let file = portfolio_file(&dir);

    nw_cmd(&file)
        .args([
            "asset",
            "add",
            "-i",
            "vti",
            "-n",
            "VTI",
            "-c",
            "etf",
            "--currency",
            "USD",
        ])
        .assert()
        .success();

    nw_cmd(&file)
        .args(["snapshot", "add", "-d", "2025-01-01"])
        .write_stdin("10000\n")
        .assert()
        .success();

    nw_cmd(&file)
        .args(["snapshot", "add", "-d", "2025-02-01"])
        .write_stdin("12000\n")
        .assert()
        .success();

    // Show table, case-insensitive category filter, historical header, JSON, CSV
    nw_cmd(&file)
        .args(["show", "--date", "2025-01-01", "--category", "ETF"])
        .assert()
        .success()
        .stdout(predicate::str::contains("NET WORTH (ETF) — 2025-01-01"))
        .stdout(predicate::str::contains("CURRENT NET WORTH").not());

    nw_cmd(&file)
        .args(["show", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"grand_total_usd\": 12000"));

    nw_cmd(&file)
        .args(["show", "--format", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2025-02-01,etf,vti,VTI,USD,12000,12000",
        ));

    // History with default range, category, and period summary
    nw_cmd(&file)
        .args(["history", "--range", "ALL", "--category", "ETF"])
        .assert()
        .success()
        .stdout(predicate::str::contains("NET WORTH HISTORY (ETF) — ALL"))
        .stdout(predicate::str::contains(
            "Period change (2025-01-01 -> 2025-02-01): +2,000.00 (+20.00%)",
        ));

    // Validate & snapshot remove
    nw_cmd(&file)
        .arg("validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("1 asset(s), 2 snapshot(s)"));

    nw_cmd(&file)
        .args(["snapshot", "remove", "-d", "2025-01-01", "-y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Snapshot removed."));

    // Malformed JSON error reports file path
    fs::write(&file, "{broken").unwrap();
    nw_cmd(&file)
        .arg("validate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("malformed JSON in"));
}
