use crate::error::NwError;
use crate::model::{Asset, HistoryReport, OutputFormat, ShowReport, Snapshot};
use comfy_table::{Cell, CellAlignment, Color, Table};
use rust_decimal::{Decimal, RoundingStrategy};
use std::io::Write;

#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayOptions {
    pub format: OutputFormat,
    pub color: bool,
}

fn round_dp_away(value: Decimal, dp: u32) -> Decimal {
    value.round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero)
}

// ---- Number formatting ----

pub fn fmt_currency(value: Decimal) -> String {
    let rounded = round_dp_away(value, 2);
    if rounded.is_zero() {
        return "0.00".to_string();
    }

    let formatted = format!("{:.2}", rounded.abs());
    let (int_part, frac_part) = formatted.split_once('.').unwrap_or((&formatted, "00"));
    let with_commas = insert_commas(int_part);

    if rounded.is_sign_negative() {
        format!("-{with_commas}.{frac_part}")
    } else {
        format!("{with_commas}.{frac_part}")
    }
}

fn insert_commas(digits: &str) -> String {
    let len = digits.len();
    digits
        .char_indices()
        .flat_map(|(i, c)| {
            let comma = (i > 0 && (len - i).is_multiple_of(3)).then_some(',');
            comma.into_iter().chain(std::iter::once(c))
        })
        .collect()
}

pub fn fmt_change(value: Decimal) -> String {
    let rounded = round_dp_away(value, 2);
    if rounded.is_zero() {
        return "0.00".to_string();
    }
    if rounded.is_sign_positive() {
        format!("+{}", fmt_currency(rounded))
    } else {
        fmt_currency(rounded)
    }
}

pub fn fmt_pct(value: Decimal) -> String {
    let rounded = round_dp_away(value, 2);
    if rounded.is_zero() {
        return "0.00%".to_string();
    }
    if rounded.is_sign_positive() {
        format!("+{rounded:.2}%")
    } else {
        format!("{rounded:.2}%")
    }
}

fn colorize_signed(text: &str, value: Decimal, enable_color: bool) -> String {
    let rounded = round_dp_away(value, 2);
    if !enable_color || rounded.is_zero() {
        return text.to_string();
    }
    if rounded.is_sign_positive() {
        format!("\x1b[32m{text}\x1b[0m")
    } else {
        format!("\x1b[31m{text}\x1b[0m")
    }
}

fn table_delta_color(value: Decimal, enable_color: bool) -> Option<Color> {
    let rounded = round_dp_away(value, 2);
    if !enable_color || rounded.is_zero() {
        return None;
    }
    if rounded.is_sign_positive() {
        Some(Color::Green)
    } else {
        Some(Color::Red)
    }
}

fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

// ---- nw show ----

pub fn render_show(
    report: &ShowReport,
    opts: DisplayOptions,
    writer: &mut impl Write,
) -> Result<(), NwError> {
    match opts.format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, report).map_err(|e| {
                NwError::SerializeJson {
                    path: "stdout".into(),
                    source: e,
                }
            })?;
            writeln!(writer)?;
        }
        OutputFormat::Csv => {
            writeln!(
                writer,
                "date,category,asset_id,asset_name,currency,native_value,usd_value"
            )?;
            for section in &report.sections {
                for row in &section.rows {
                    writeln!(
                        writer,
                        "{},{},{},{},{},{},{}",
                        report.date,
                        csv_escape(&row.category),
                        csv_escape(&row.asset_id),
                        csv_escape(&row.asset_name),
                        csv_escape(&row.currency),
                        round_dp_away(row.native_value, 2),
                        round_dp_away(row.usd_value, 2)
                    )?;
                }
            }
        }
        OutputFormat::Table => render_show_table(report, opts, writer)?,
    }
    Ok(())
}

fn render_show_table(
    report: &ShowReport,
    opts: DisplayOptions,
    writer: &mut impl Write,
) -> Result<(), NwError> {
    match (&report.category_filter, report.is_latest) {
        (Some(cat), true) => writeln!(
            writer,
            "CURRENT NET WORTH ({}) — {}",
            cat.to_uppercase(),
            report.date
        )?,
        (Some(cat), false) => {
            writeln!(
                writer,
                "NET WORTH ({}) — {}",
                cat.to_uppercase(),
                report.date
            )?;
        }
        (None, true) => writeln!(writer, "CURRENT NET WORTH — {}", report.date)?,
        (None, false) => writeln!(writer, "NET WORTH — {}", report.date)?,
    }

    if report.sections.is_empty() {
        writeln!(writer)?;
        if let Some(cat) = &report.category_filter {
            writeln!(writer, "No entries found for category '{cat}'.")?;
        } else {
            writeln!(writer, "Snapshot has no entries.")?;
        }
        return Ok(());
    }

    for section in &report.sections {
        writeln!(writer)?;
        writeln!(writer, "{}", section.category.to_uppercase())?;

        let mut table = Table::new();
        table.load_preset(comfy_table::presets::NOTHING);
        table.set_header(vec!["  Name", "Currency", "Value (native)", "Value (USD)"]);

        for row in &section.rows {
            table.add_row(vec![
                Cell::new(format!("  {}", row.asset_name)),
                Cell::new(&row.currency),
                Cell::new(fmt_currency(row.native_value)).set_alignment(CellAlignment::Right),
                Cell::new(fmt_currency(row.usd_value)).set_alignment(CellAlignment::Right),
            ]);
        }
        table.add_row(vec![
            Cell::new("  Subtotal"),
            Cell::new(""),
            Cell::new(""),
            Cell::new(fmt_currency(section.subtotal_usd)).set_alignment(CellAlignment::Right),
        ]);

        writeln!(writer, "{table}")?;
    }

    writeln!(writer)?;
    let total_str = fmt_currency(report.grand_total_usd);
    if let Some(delta) = &report.delta_from_previous {
        let change_part = match delta.change_pct {
            Some(pct) => format!("{}, {}", fmt_change(delta.change_usd), fmt_pct(pct)),
            None => fmt_change(delta.change_usd),
        };
        let styled_delta = colorize_signed(&change_part, delta.change_usd, opts.color);
        writeln!(
            writer,
            "TOTAL  {total_str}  ({styled_delta} vs {})",
            delta.previous_date
        )?;
    } else {
        writeln!(writer, "TOTAL  {total_str}")?;
    }

    if report.category_filter.is_none() && !report.allocation.is_empty() {
        let label_width = report
            .allocation
            .iter()
            .map(|a| a.category.len())
            .max()
            .unwrap_or(0)
            .max(12);
        writeln!(writer)?;
        writeln!(writer, "ALLOCATION")?;
        for alloc in &report.allocation {
            writeln!(
                writer,
                "  {:<width$} {:>6.1}%",
                alloc.category.to_uppercase(),
                round_dp_away(alloc.pct, 1),
                width = label_width
            )?;
        }
    }

    Ok(())
}

// ---- nw history ----

pub fn render_history(
    report: &HistoryReport,
    opts: DisplayOptions,
    writer: &mut impl Write,
) -> Result<(), NwError> {
    match opts.format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, report).map_err(|e| {
                NwError::SerializeJson {
                    path: "stdout".into(),
                    source: e,
                }
            })?;
            writeln!(writer)?;
        }
        OutputFormat::Csv => {
            writeln!(writer, "date,total_usd,change_usd,change_pct")?;
            for row in &report.rows {
                let change_usd = row
                    .change_usd
                    .map_or_default(|v| round_dp_away(v, 2).to_string());
                let change_pct = row
                    .change_pct
                    .map_or_default(|v| round_dp_away(v, 2).to_string());
                writeln!(
                    writer,
                    "{},{},{},{}",
                    row.date,
                    round_dp_away(row.total_usd, 2),
                    change_usd,
                    change_pct
                )?;
            }
        }
        OutputFormat::Table => render_history_table(report, opts, writer)?,
    }
    Ok(())
}

fn render_history_table(
    report: &HistoryReport,
    opts: DisplayOptions,
    writer: &mut impl Write,
) -> Result<(), NwError> {
    match &report.category_filter {
        Some(cat) => writeln!(
            writer,
            "NET WORTH HISTORY ({}) — {}",
            cat.to_uppercase(),
            report.range
        )?,
        None => writeln!(writer, "NET WORTH HISTORY — {}", report.range)?,
    }
    writeln!(writer)?;

    let mut table = Table::new();
    table.load_preset(comfy_table::presets::NOTHING);
    table.set_header(vec!["Date", "Total (USD)", "Change (USD)", "Change %"]);

    for row in &report.rows {
        let change_usd_str = row.change_usd.map_or_else(|| "—".to_string(), fmt_change);
        let change_pct_str = row.change_pct.map_or_else(|| "—".to_string(), fmt_pct);

        let mut change_usd_cell = Cell::new(change_usd_str).set_alignment(CellAlignment::Right);
        let mut change_pct_cell = Cell::new(change_pct_str).set_alignment(CellAlignment::Right);
        if let Some(color) = row
            .change_usd
            .and_then(|v| table_delta_color(v, opts.color))
        {
            change_usd_cell = change_usd_cell.fg(color);
            change_pct_cell = change_pct_cell.fg(color);
        }

        table.add_row(vec![
            Cell::new(row.date),
            Cell::new(fmt_currency(row.total_usd)).set_alignment(CellAlignment::Right),
            change_usd_cell,
            change_pct_cell,
        ]);
    }

    writeln!(writer, "{table}")?;

    if let Some(summary) = &report.summary {
        let delta_text = match summary.change_pct {
            Some(pct) => format!("{} ({})", fmt_change(summary.change_usd), fmt_pct(pct)),
            None => fmt_change(summary.change_usd),
        };
        let styled = colorize_signed(&delta_text, summary.change_usd, opts.color);
        writeln!(writer)?;
        writeln!(
            writer,
            "Period change ({} -> {}): {styled}",
            summary.start_date, summary.end_date,
        )?;
    }

    Ok(())
}

// ---- nw asset list ----

pub fn render_asset_list(
    assets: &[&Asset],
    opts: DisplayOptions,
    writer: &mut impl Write,
) -> Result<(), NwError> {
    match opts.format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, assets).map_err(|e| {
                NwError::SerializeJson {
                    path: "stdout".into(),
                    source: e,
                }
            })?;
            writeln!(writer)?;
        }
        OutputFormat::Csv => {
            writeln!(writer, "id,name,category,currency,archived")?;
            for asset in assets {
                writeln!(
                    writer,
                    "{},{},{},{},{}",
                    csv_escape(&asset.id),
                    csv_escape(&asset.name),
                    csv_escape(&asset.category),
                    csv_escape(&asset.currency),
                    asset.archived
                )?;
            }
        }
        OutputFormat::Table => {
            if assets.is_empty() {
                writeln!(writer, "No assets yet.")?;
                return Ok(());
            }

            let show_status = assets.iter().any(|a| a.archived);
            let mut table = Table::new();
            table.load_preset(comfy_table::presets::NOTHING);
            if show_status {
                table.set_header(vec!["ID", "Name", "Category", "Currency", "Status"]);
                for asset in assets {
                    let status = if asset.archived { "archived" } else { "active" };
                    table.add_row(vec![
                        asset.id.as_str(),
                        asset.name.as_str(),
                        asset.category.as_str(),
                        asset.currency.as_str(),
                        status,
                    ]);
                }
            } else {
                table.set_header(vec!["ID", "Name", "Category", "Currency"]);
                for asset in assets {
                    table.add_row(vec![
                        asset.id.as_str(),
                        asset.name.as_str(),
                        asset.category.as_str(),
                        asset.currency.as_str(),
                    ]);
                }
            }

            writeln!(writer, "{table}")?;
        }
    }
    Ok(())
}

// ---- nw snapshot list ----

pub fn render_snapshot_list(
    snapshots: &[Snapshot],
    opts: DisplayOptions,
    writer: &mut impl Write,
) -> Result<(), NwError> {
    match opts.format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, snapshots).map_err(|e| {
                NwError::SerializeJson {
                    path: "stdout".into(),
                    source: e,
                }
            })?;
            writeln!(writer)?;
        }
        OutputFormat::Csv => {
            writeln!(writer, "date,entries,fx_rates")?;
            for snapshot in snapshots {
                let rates_str = format_fx_rates(snapshot);
                writeln!(
                    writer,
                    "{},{},{}",
                    snapshot.date,
                    snapshot.entries.len(),
                    csv_escape(&rates_str)
                )?;
            }
        }
        OutputFormat::Table => {
            if snapshots.is_empty() {
                writeln!(writer, "No snapshots yet.")?;
                return Ok(());
            }

            let mut table = Table::new();
            table.load_preset(comfy_table::presets::NOTHING);
            table.set_header(vec!["Date", "Entries", "FX Rates"]);

            for snapshot in snapshots {
                table.add_row(vec![
                    snapshot.date.to_string(),
                    snapshot.entries.len().to_string(),
                    format_fx_rates(snapshot),
                ]);
            }

            writeln!(writer, "{table}")?;
        }
    }
    Ok(())
}

fn format_fx_rates(snapshot: &Snapshot) -> String {
    if snapshot.rates.is_empty() {
        "—".to_string()
    } else {
        snapshot
            .rates
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests;
