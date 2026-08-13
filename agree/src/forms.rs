use crate::ai::Intent;
use crate::client::Client;
use crate::models::{BillingContact, Contact, RecurringOptions};
use crate::money::Money;
use anyhow::{bail, Context, Result};
use chrono::{Duration, Utc};
use console::style;
use dialoguer::{Confirm, Input, Select};
use serde::Serialize;

/// A fully specified invoice, ready to send. Nothing reaches this struct without
/// either passing validation or being typed by the user.
#[derive(Debug, Serialize)]
pub struct NewInvoice {
    pub amount: Money,
    pub due_at: String,
    pub scheduled_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_contact: Option<BillingContact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurring_options: Option<RecurringOptions>,
}

/// Ask for anything the model could not supply, then show the whole thing back.
///
/// Every gap is a question rather than a guess, which is what makes a weak model
/// safe to use: the worst case is being asked more questions.
pub async fn build_invoice(
    api: &Client,
    intent: &Intent,
    currency: &str,
) -> Result<Option<NewInvoice>> {
    let amount = match intent.money(currency) {
        Some(money) => {
            println!("  amount   {}", style(money.display()).green());
            money
        }
        None => ask_amount(currency)?,
    };

    let contact = resolve_contact(api, intent.payee.as_deref()).await?;
    let contact_label = contact.label.clone();
    let mut recurring = confirm_cadence(intent)?;

    // The due date has to be settled before the schedule can be, because a weekly
    // repeat needs a weekday and a monthly one needs a day of the month — the API
    // rejects the request outright without them.
    let due_at = ask_due_date(intent.due_date.as_deref())?;
    if let Some(options) = recurring.as_mut() {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&due_at[..10], "%Y-%m-%d") {
            options.anchor_to(date);
        }
    }

    let memo = match &intent.memo {
        Some(text) if !text.is_empty() => Some(text.clone()),
        _ => None,
    };

    let invoice = NewInvoice {
        amount,
        scheduled_at: send_date(&due_at),
        due_at,
        memo,
        billing_contact: contact.new_contact,
        contact_id: contact.existing_id,
        recurring_options: recurring,
    };

    preview(&invoice, &contact_label);

    let go = Confirm::new()
        .with_prompt("Create this invoice?")
        .default(false)
        .interact()
        .context("No answer given")?;

    Ok(go.then_some(invoice))
}

struct ResolvedContact {
    existing_id: Option<String>,
    new_contact: Option<BillingContact>,
    /// How to describe the payer in the review block — a uuid tells nobody anything
    label: String,
}

/// Turn a spoken name into a real contact. The API cannot search by name, so this
/// pages contacts and matches locally, then always confirms — a name is never
/// assumed to mean one person.
async fn resolve_contact(api: &Client, payee: Option<&str>) -> Result<ResolvedContact> {
    let all: Vec<Contact> = api.list_all("/api/v1/contacts", &[], 0).await?;

    let matches: Vec<&Contact> = match payee {
        Some(name) => all.iter().filter(|c| c.matches(name)).collect(),
        None => Vec::new(),
    };

    if matches.len() == 1 {
        let found = matches[0];
        println!("  payee    {}", style(found.label()).green());
        return Ok(ResolvedContact {
            existing_id: Some(found.id.clone()),
            new_contact: None,
            label: found.label(),
        });
    }

    if matches.len() > 1 {
        println!(
            "\n  {}",
            style(format!("{} contacts match \"{}\"", matches.len(), payee.unwrap_or(""))).yellow()
        );
        let labels: Vec<String> = matches.iter().map(|c| c.label()).collect();
        let idx = Select::new()
            .with_prompt("Which one?")
            .items(&labels)
            .default(0)
            .interact()
            .context("No contact chosen")?;
        return Ok(ResolvedContact {
            existing_id: Some(matches[idx].id.clone()),
            new_contact: None,
            label: matches[idx].label(),
        });
    }

    // Nothing matched — offer to create, or pick from everyone.
    if let Some(name) = payee {
        println!("\n  {}", style(format!("No contact matches \"{name}\"")).yellow());
    }

    let options = ["Create a new contact", "Pick from all contacts", "Cancel"];
    let choice = Select::new()
        .with_prompt("What next?")
        .items(&options)
        .default(0)
        .interact()
        .context("No choice made")?;

    match choice {
        0 => {
            let contact = ask_new_contact(payee)?;
            let label = format!(
                "{} <{}>",
                contact.name.clone().unwrap_or_default(),
                contact.email
            );
            Ok(ResolvedContact { existing_id: None, new_contact: Some(contact), label })
        }
        1 => {
            if all.is_empty() {
                bail!("There are no contacts to pick from");
            }
            let labels: Vec<String> = all.iter().map(|c| c.label()).collect();
            let idx = Select::new()
                .with_prompt("Contact")
                .items(&labels)
                .default(0)
                .interact()
                .context("No contact chosen")?;
            Ok(ResolvedContact {
                existing_id: Some(all[idx].id.clone()),
                new_contact: None,
                label: all[idx].label(),
            })
        }
        _ => bail!("Cancelled"),
    }
}

/// The email is the one field a model must never invent, so it is always typed.
fn ask_new_contact(suggested_name: Option<&str>) -> Result<BillingContact> {
    println!();
    let name: String = Input::new()
        .with_prompt("Name")
        .with_initial_text(suggested_name.unwrap_or_default())
        .interact_text()
        .context("No name given")?;

    let email: String = Input::new()
        .with_prompt("Email")
        .validate_with(|input: &String| match input.contains('@') && input.contains('.') {
            true => Ok(()),
            false => Err("that does not look like an email address"),
        })
        .interact_text()
        .context("No email given")?;

    let company: String = Input::new()
        .with_prompt("Company (enter to skip)")
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    Ok(BillingContact {
        name: Some(name.trim().to_string()),
        email: email.trim().to_string(),
        company: (!company.trim().is_empty()).then(|| company.trim().to_string()),
        title: None,
    })
}

fn ask_amount(currency: &str) -> Result<Money> {
    let raw: String = Input::new()
        .with_prompt("Amount")
        .validate_with(|input: &String| Money::parse(input, "USD").map(|_| ()).map_err(|e| e.to_string()))
        .interact_text()
        .context("No amount given")?;
    Money::parse(&raw, currency)
}

/// Confirm the repeat schedule rather than assuming it — "weekly" heard wrong is
/// an expensive mistake.
fn confirm_cadence(intent: &Intent) -> Result<Option<RecurringOptions>> {
    if let Some(unit) = intent.unit() {
        let frequency = intent.frequency();
        let described = match frequency {
            1 => format!("every {unit}"),
            n => format!("every {n} {unit}s"),
        };
        println!("  repeats  {}", style(&described).green());
        return Ok(Some(RecurringOptions::every(&unit, frequency)));
    }

    let options = ["One-off invoice", "Every week", "Every month"];
    let idx = Select::new()
        .with_prompt("How often?")
        .items(&options)
        .default(0)
        .interact()
        .context("No choice made")?;

    Ok(match idx {
        1 => Some(RecurringOptions::every("week", 1)),
        2 => Some(RecurringOptions::every("month", 1)),
        _ => None,
    })
}

/// Dates are never accepted silently, even when well formed.
///
/// Models resolve relative dates badly — "next friday" came back as a Wednesday
/// in testing. A date is always shown pre-filled for the user to accept or fix,
/// with the weekday spelled out so a wrong one is obvious at a glance.
fn ask_due_date(from_model: Option<&str>) -> Result<String> {
    let suggestion = from_model
        .filter(|d| looks_like_date(d))
        .map(String::from)
        .unwrap_or_else(|| (Utc::now() + Duration::days(30)).format("%Y-%m-%d").to_string());

    let entered: String = Input::new()
        .with_prompt(format!("Due date{}", weekday_hint(&suggestion)))
        .with_initial_text(&suggestion)
        .validate_with(|input: &String| match looks_like_date(input) {
            true => Ok(()),
            false => Err("use YYYY-MM-DD"),
        })
        .interact_text()
        .context("No due date given")?;

    Ok(format!("{}T00:00:00Z", entered.trim()))
}

/// When the invoice is sent. The API refuses a send date later than the due date
/// ("Invoice date cannot be after due date"), so a backdated invoice sends on the
/// day it was due rather than today.
fn send_date(due_at: &str) -> String {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    match due_at < now.as_str() {
        true => due_at.to_string(),
        false => now,
    }
}

/// " (Friday)" — makes a model's wrong weekday obvious before it is accepted
fn weekday_hint(date: &str) -> String {
    use chrono::NaiveDate;
    match NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(parsed) => format!(" ({})", parsed.format("%A")),
        Err(_) => String::new(),
    }
}

fn looks_like_date(value: &str) -> bool {
    let value = value.trim();
    value.len() == 10
        && value.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        })
}

/// Everything that is about to happen, in one block, before anything is sent.
fn preview(invoice: &NewInvoice, payer: &str) {
    let cadence = invoice
        .recurring_options
        .as_ref()
        .map(|r| format!("{}, forever", r.describe()))
        .unwrap_or_else(|| "one-off".into());

    println!();
    println!("{}", style("  ── Review ─────────────────────────────").dim());
    println!("  {:<10} {}", "Amount", style(invoice.amount.display()).bold());
    println!("  {:<10} {}", "Billed to", payer);
    println!("  {:<10} {}", "Repeats", cadence);
    println!("  {:<10} {}", "Sends", &invoice.scheduled_at[..10]);
    println!("  {:<10} {}", "Due", &invoice.due_at[..10]);
    if invoice.scheduled_at[..10] < *chrono::Utc::now().format("%Y-%m-%d").to_string() {
        println!(
            "  {:<10} {}",
            "",
            style("backdated — this invoice is already overdue").yellow()
        );
    }
    if let Some(memo) = &invoice.memo {
        println!("  {:<10} {}", "Memo", memo);
    }
    println!("{}", style("  ───────────────────────────────────────").dim());
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_date_shape() {
        assert!(looks_like_date("2026-07-08"));
        assert!(!looks_like_date("2026-7-8"));
        assert!(!looks_like_date("July 8th"));
        assert!(!looks_like_date("2026-07-08T00:00:00Z"));
    }

    #[test]
    fn serialises_only_what_is_set() {
        let invoice = NewInvoice {
            amount: Money::from_cents(500_000, "USD"),
            due_at: "2026-07-08T00:00:00Z".into(),
            scheduled_at: "2026-07-01T00:00:00Z".into(),
            memo: None,
            billing_contact: None,
            contact_id: Some("abc".into()),
            recurring_options: Some(RecurringOptions::every("week", 1)),
        };
        let json = serde_json::to_value(&invoice).unwrap();
        assert_eq!(json["amount"]["amount"], 500_000);
        assert_eq!(json["contact_id"], "abc");
        assert_eq!(json["recurring_options"]["repeat_unit"], "week");
        // Mutually exclusive with contact_id — must be absent, not null
        assert!(json.get("billing_contact").is_none());
        assert!(json.get("memo").is_none());
    }
}

#[cfg(test)]
mod date_tests {
    use super::*;

    #[test]
    fn spells_out_the_weekday_so_a_wrong_one_shows() {
        // The exact case observed: a model answered "next friday" with a Wednesday.
        assert_eq!(weekday_hint("2026-08-19"), " (Wednesday)");
        assert_eq!(weekday_hint("2026-08-21"), " (Friday)");
        assert_eq!(weekday_hint("not-a-date"), "");
    }
}

#[cfg(test)]
mod send_date_tests {
    use super::*;

    #[test]
    fn backdated_invoice_sends_on_its_due_date() {
        // The API refuses "Invoice date cannot be after due date".
        let due = "2026-07-08T00:00:00Z";
        assert_eq!(send_date(due), due, "a past due date must also be the send date");
    }

    #[test]
    fn future_invoice_sends_today() {
        let due = "2099-01-01T00:00:00Z";
        let sent = send_date(due);
        assert!(sent < due.to_string(), "send date must not be after the due date");
        assert!(sent.starts_with(&Utc::now().format("%Y-%m-%d").to_string()));
    }

    #[test]
    fn send_is_never_after_due() {
        for due in ["2020-01-01T00:00:00Z", "2026-07-08T00:00:00Z", "2099-12-31T00:00:00Z"] {
            assert!(send_date(due) <= due.to_string(), "violated for {due}");
        }
    }
}
