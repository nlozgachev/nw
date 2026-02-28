use std::collections::BTreeMap;
use comfy_table::{Cell, Table};
use crate::model::{Asset, HistoryRow, ShowRow, Snapshot};

// ---- Number formatting ----

fn fmt_currency(value: f64) -> String {
    let abs = value.abs();
    let mut int_part = abs.floor() as u64;
    let mut frac = ((abs - abs.floor()) * 100.0).round() as u64;
    if frac == 100 {
        frac = 0;
        int_part += 1;
    }

    let int_str = fmt_with_commas(int_part);

    if value < 0.0 {
        format!("-{}.{:02}", int_str, frac)
    } else {
        format!("{}.{:02}", int_str, frac)
    }
}

fn fmt_with_commas(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    s.chars()
        .enumerate()
        .flat_map(|(i, c)| {
            let comma = (i > 0 && (len - i).is_multiple_of(3)).then_some(',');
            comma.into_iter().chain(std::iter::once(c))
        })
        .collect()
}

fn fmt_change(value: f64) -> String {
    if value >= 0.0 {
        format!("+{}", fmt_currency(value))
    } else {
        fmt_currency(value)
    }
}

fn fmt_pct(value: f64) -> String {
    if value >= 0.0 {
        format!("+{:.2}%", value)
    } else {
        format!("{:.2}%", value)
    }
}

// ---- nw show ----

pub fn print_show(
    rows: Vec<ShowRow>,
    grand_total: f64,
    allocation: Vec<(String, f64)>,
    date: &str,
    category_filter: Option<&str>,
) {
    if category_filter.is_some() {
        println!("NET WORTH — {}", date);
    } else {
        println!("CURRENT NET WORTH — {}", date);
    }

    // Group rows by category (BTreeMap for stable alphabetical order)
    let mut by_category: BTreeMap<String, Vec<ShowRow>> = BTreeMap::new();
    for row in rows {
        by_category.entry(row.category.clone()).or_default().push(row);
    }

    for (category, cat_rows) in &by_category {
        println!();
        println!("{}", category.to_uppercase());

        let mut table = Table::new();
        table.load_preset(comfy_table::presets::NOTHING);
        table.set_header(vec!["  Name", "Currency", "Value (native)", "Value (USD)"]);

        let mut subtotal = 0.0;
        for row in cat_rows {
            subtotal += row.usd_value;
            table.add_row(vec![
                Cell::new(format!("  {}", row.asset_name)),
                Cell::new(&row.currency),
                Cell::new(fmt_currency(row.native_value)).set_alignment(
                    comfy_table::CellAlignment::Right,
                ),
                Cell::new(fmt_currency(row.usd_value))
                    .set_alignment(comfy_table::CellAlignment::Right),
            ]);
        }
        table.add_row(vec![
            Cell::new("  Subtotal"),
            Cell::new(""),
            Cell::new(""),
            Cell::new(fmt_currency(subtotal)).set_alignment(comfy_table::CellAlignment::Right),
        ]);

        println!("{table}");
    }

    println!();
    println!("TOTAL  {}", fmt_currency(grand_total));

    if category_filter.is_none() && !allocation.is_empty() {
        println!();
        println!("ALLOCATION");
        for (cat, pct) in &allocation {
            println!("  {:<12} {:>6.1}%", cat.to_uppercase(), pct);
        }
    }
}

// ---- nw history ----

pub fn print_history(rows: Vec<HistoryRow>, range_label: &str) {
    println!("NET WORTH HISTORY — {}", range_label);
    println!();

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::NOTHING);
    table.set_header(vec!["Date", "Total (USD)", "Change (USD)", "Change %"]);

    for row in rows {
        let change_usd = row
            .change_usd
            .map(fmt_change)
            .unwrap_or_else(|| "—".to_string());
        let change_pct = row
            .change_pct
            .map(fmt_pct)
            .unwrap_or_else(|| "—".to_string());
        table.add_row(vec![
            Cell::new(&row.date),
            Cell::new(fmt_currency(row.total_usd))
                .set_alignment(comfy_table::CellAlignment::Right),
            Cell::new(change_usd).set_alignment(comfy_table::CellAlignment::Right),
            Cell::new(change_pct).set_alignment(comfy_table::CellAlignment::Right),
        ]);
    }

    println!("{table}");
}

// ---- nw asset list ----

pub fn print_asset_list(assets: &[Asset]) {
    if assets.is_empty() {
        println!("No assets yet.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::NOTHING);
    table.set_header(vec!["ID", "Name", "Category", "Currency"]);

    for asset in assets {
        table.add_row(vec![&asset.id, &asset.name, &asset.category, &asset.currency]);
    }

    println!("{table}");
}

// ---- nw snapshot list ----

pub fn print_snapshot_list(snapshots: &[Snapshot]) {
    if snapshots.is_empty() {
        println!("No snapshots yet.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::NOTHING);
    table.set_header(vec!["Date", "Entries", "Currencies"]);

    for snapshot in snapshots {
        let mut currencies: Vec<&str> = snapshot.rates.keys().map(|s| s.as_str()).collect();
        currencies.sort();
        let currencies_str = if currencies.is_empty() {
            "USD only".to_string()
        } else {
            format!("USD, {}", currencies.join(", "))
        };
        table.add_row(vec![
            snapshot.date.clone(),
            snapshot.entries.len().to_string(),
            currencies_str,
        ]);
    }

    println!("{table}");
}