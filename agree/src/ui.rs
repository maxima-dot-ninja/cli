use crate::models::{Agreement, Contact, Customer, Invoice, Template};
use anyhow::Result;
use console::style;
use serde::Serialize;

pub fn print_header() {
    println!();
    println!("{}", style("┌─────────────────────────────────────────────┐").cyan());
    println!("{}", style("│  agree — invoices, agreements, contacts     │").cyan());
    println!("{}", style("└─────────────────────────────────────────────┘").cyan());
    println!();
}

/// Statuses carry money meaning, so they get colour: paid is good, failed is not.
fn status_style(status: &str) -> String {
    let text = status.to_string();
    match status {
        "paid" => style(text).green().to_string(),
        "failed" | "canceled" => style(text).red().to_string(),
        "due" | "sent" => style(text).yellow().to_string(),
        "draft" | "created" => style(text).dim().to_string(),
        _ => text,
    }
}

fn short(value: &Option<String>, width: usize) -> String {
    let text = value.clone().unwrap_or_else(|| "—".into());
    match text.chars().count() > width {
        true => format!("{}…", text.chars().take(width - 1).collect::<String>()),
        false => text,
    }
}

/// Dates come back as ISO8601; only the day is useful in a list.
fn day(value: &Option<String>) -> String {
    value
        .as_ref()
        .map(|d| d.chars().take(10).collect::<String>())
        .unwrap_or_else(|| "—".into())
}

fn dump<T: Serialize>(rows: &[T]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&serde_json::to_value(rows)?)?);
    Ok(())
}

fn empty(what: &str) {
    println!("\n  {}\n", style(format!("No {what} found.")).dim());
}

pub fn print_invoices(rows: &[Invoice], as_json: bool) -> Result<()> {
    if as_json {
        return dump_raw(rows);
    }
    if rows.is_empty() {
        empty("invoices");
        return Ok(());
    }

    println!();
    println!(
        "  {:<12} {:>12}  {:<24} {:<12} {}",
        style("STATUS").dim(),
        style("AMOUNT").dim(),
        style("PAYER").dim(),
        style("DUE").dim(),
        style("REPEATS").dim()
    );

    for invoice in rows {
        let status = invoice.status.clone().unwrap_or_else(|| "—".into());
        println!(
            "  {:<12} {:>12}  {:<24} {:<12} {}",
            status_style(&status),
            invoice.amount_display(),
            short(&Some(invoice.payer()), 24),
            day(&invoice.due_at),
            style(invoice.cadence()).dim()
        );
    }
    println!("\n  {} invoices\n", rows.len());
    Ok(())
}

pub fn print_contacts(rows: &[Contact], as_json: bool) -> Result<()> {
    if as_json {
        return dump_raw(rows);
    }
    if rows.is_empty() {
        empty("contacts");
        return Ok(());
    }

    println!();
    for contact in rows {
        println!("  {}", contact.label());
    }
    println!("\n  {} contacts\n", rows.len());
    Ok(())
}

pub fn print_customers(rows: &[Customer], as_json: bool) -> Result<()> {
    if as_json {
        return dump_raw(rows);
    }
    if rows.is_empty() {
        empty("customers");
        return Ok(());
    }

    println!();
    for customer in rows {
        println!(
            "  {:<32} {}",
            short(&customer.name, 32),
            style(customer.business_type.clone().unwrap_or_default()).dim()
        );
    }
    println!("\n  {} customers\n", rows.len());
    Ok(())
}

pub fn print_agreements(rows: &[Agreement], as_json: bool) -> Result<()> {
    if as_json {
        return dump_raw(rows);
    }
    if rows.is_empty() {
        empty("agreements");
        return Ok(());
    }

    println!();
    println!(
        "  {:<12} {:<36} {:<12} {}",
        style("STATUS").dim(),
        style("NAME").dim(),
        style("STARTS").dim(),
        style("MODE").dim()
    );
    for agreement in rows {
        println!(
            "  {:<12} {:<36} {:<12} {}",
            agreement.status.clone().unwrap_or_else(|| "—".into()),
            short(&agreement.name, 36),
            day(&agreement.starts_at),
            style(agreement.delivery_mode.clone().unwrap_or_default()).dim()
        );
    }
    println!("\n  {} agreements\n", rows.len());
    Ok(())
}

pub fn print_templates(rows: &[Template], as_json: bool) -> Result<()> {
    if as_json {
        return dump_raw(rows);
    }
    if rows.is_empty() {
        empty("templates");
        return Ok(());
    }

    println!();
    for template in rows {
        println!("  {}  {}", style(&template.id).dim(), short(&template.name, 48));
    }
    println!("\n  {} templates\n", rows.len());
    Ok(())
}

fn dump_raw<T: Serialize>(rows: &[T]) -> Result<()> {
    dump(rows)
}

pub fn spinner(message: &str) -> indicatif::ProgressBar {
    let bar = indicatif::ProgressBar::new_spinner();
    bar.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    bar.set_message(message.to_string());
    bar.enable_steady_tick(std::time::Duration::from_millis(80));
    bar
}

pub fn success(message: &str) {
    println!("\n  {} {}", style("✓").green().bold(), message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_long_values() {
        assert_eq!(short(&Some("short".into()), 10), "short");
        assert_eq!(short(&Some("a".repeat(20)), 10), format!("{}…", "a".repeat(9)));
        assert_eq!(short(&None, 10), "—");
    }

    #[test]
    fn dates_shorten_to_the_day() {
        assert_eq!(day(&Some("2026-07-08T00:00:00Z".into())), "2026-07-08");
        assert_eq!(day(&None), "—");
    }
}
