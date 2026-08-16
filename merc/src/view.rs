//! Turning a JSON reply into something readable.
//!
//! Layouts are a table, not a function per resource: a row names the schema and
//! the columns worth showing, and one renderer draws all of them. A schema with
//! no row falls back to indented JSON, so a new Mercury endpoint prints usefully
//! on the day it appears — just without a hand-picked column order.

use crate::money::Money;
use crate::ops::Op;
use console::style;
use serde_json::Value;

#[derive(Clone, Copy, PartialEq)]
pub enum Cell {
    Text,
    /// Right-aligned, grouped, always two decimal places
    Money,
    /// ISO timestamp cut down to the day
    Day,
    /// Coloured by whether it is good news
    Status,
    /// Dimmed, because ids are for copying, not reading. This one identifies
    /// the row itself, which is what a picker hands back.
    Id,
    /// An id belonging to something else — dimmed the same way, but never
    /// mistaken for this row's identity.
    Ref,
}

pub struct Col {
    pub header: &'static str,
    /// Dotted paths reach into nested objects: `organization.legalBusinessName`
    pub field: &'static str,
    pub width: usize,
    pub cell: Cell,
}

pub struct View {
    pub schema: &'static str,
    pub cols: &'static [Col],
}

const fn col(header: &'static str, field: &'static str, width: usize, cell: Cell) -> Col {
    Col { header, field, width, cell }
}

/// The resources people look at often enough to deserve a chosen layout.
pub const VIEWS: &[View] = &[
    View {
        schema: "Account",
        cols: &[
            col("NAME", "name", 26, Cell::Text),
            col("TYPE", "type", 12, Cell::Text),
            col("BALANCE", "currentBalance", 14, Cell::Money),
            col("AVAILABLE", "availableBalance", 14, Cell::Money),
            col("STATUS", "status", 10, Cell::Status),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "Transaction",
        cols: &[
            col("DATE", "createdAt", 10, Cell::Day),
            col("COUNTERPARTY", "counterpartyName", 28, Cell::Text),
            col("AMOUNT", "amount", 14, Cell::Money),
            col("STATUS", "status", 10, Cell::Status),
            col("KIND", "kind", 16, Cell::Text),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "TreasuryTxn",
        cols: &[
            col("DAY", "canonicalDay", 10, Cell::Day),
            col("DESCRIPTION", "description", 34, Cell::Text),
            col("AMOUNT", "amount", 14, Cell::Money),
            col("BALANCE", "balance", 14, Cell::Money),
            col("TYPE", "type", 14, Cell::Text),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "Card",
        cols: &[
            col("LAST 4", "lastFour", 7, Cell::Text),
            col("NAME ON CARD", "nameOnCard", 24, Cell::Text),
            col("NICKNAME", "nickname", 18, Cell::Text),
            col("TYPE", "type", 9, Cell::Text),
            col("KIND", "kind", 7, Cell::Text),
            col("STATUS", "status", 10, Cell::Status),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "AccountCard",
        cols: &[
            col("LAST 4", "lastFourDigits", 7, Cell::Text),
            col("NAME ON CARD", "nameOnCard", 24, Cell::Text),
            col("NETWORK", "network", 10, Cell::Text),
            col("STATUS", "status", 10, Cell::Status),
            col("ID", "cardId", 38, Cell::Id),
        ],
    },
    View {
        schema: "RecipientInfo",
        cols: &[
            col("NAME", "name", 28, Cell::Text),
            col("EMAIL", "contactEmail", 30, Cell::Text),
            col("PAYS BY", "defaultPaymentMethod", 14, Cell::Text),
            col("STATUS", "status", 10, Cell::Status),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "TreasuryAccount",
        cols: &[
            col("BALANCE", "currentBalance", 14, Cell::Money),
            col("AVAILABLE", "availableBalance", 14, Cell::Money),
            col("RETURNS", "netReturns", 14, Cell::Money),
            col("STATUS", "status", 10, Cell::Status),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "CreditAccount",
        cols: &[
            col("BALANCE", "currentBalance", 14, Cell::Money),
            col("AVAILABLE", "availableBalance", 14, Cell::Money),
            col("STATUS", "status", 10, Cell::Status),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "DepositoryAccountStatement",
        cols: &[
            col("FROM", "startDate", 10, Cell::Day),
            col("TO", "endDate", 10, Cell::Day),
            col("ENDING BALANCE", "endingBalance", 16, Cell::Money),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "TreasuryStatement",
        cols: &[
            col("FROM", "periodStart", 10, Cell::Day),
            col("TO", "periodEnd", 10, Cell::Day),
            col("TYPE", "documentType", 16, Cell::Text),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "SendMoneyApprovalRequestResponse",
        cols: &[
            col("REQUESTED", "createdAt", 10, Cell::Day),
            col("AMOUNT", "amount", 14, Cell::Money),
            col("STATUS", "status", 12, Cell::Status),
            col("MEMO", "memo", 26, Cell::Text),
            col("ID", "requestId", 38, Cell::Id),
        ],
    },
    View {
        schema: "ApiV1ArInvoicesData",
        cols: &[
            col("NUMBER", "invoiceNumber", 12, Cell::Text),
            col("DUE", "dueDate", 10, Cell::Day),
            col("AMOUNT", "amount", 14, Cell::Money),
            col("STATUS", "status", 12, Cell::Status),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "ApiV1ArCustomerResponseData",
        cols: &[
            col("NAME", "name", 30, Cell::Text),
            col("EMAIL", "email", 32, Cell::Text),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "CategoryData",
        cols: &[col("NAME", "name", 40, Cell::Text), col("ID", "id", 38, Cell::Id)],
    },
    View {
        schema: "MerchantInfo",
        cols: &[col("NAME", "name", 40, Cell::Text), col("ID", "id", 38, Cell::Id)],
    },
    View {
        schema: "UserDetails",
        cols: &[
            col("FIRST", "firstName", 16, Cell::Text),
            col("LAST", "lastName", 20, Cell::Text),
            col("EMAIL", "email", 32, Cell::Text),
            col("ROLE", "organizationRole", 14, Cell::Text),
            col("ID", "userId", 38, Cell::Id),
        ],
    },
    View {
        schema: "ApiWebhookResponse",
        cols: &[
            col("URL", "url", 44, Cell::Text),
            col("STATUS", "status", 10, Cell::Status),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "ApiEventResponse",
        cols: &[
            col("WHEN", "occurredAt", 10, Cell::Day),
            col("RESOURCE", "resourceType", 18, Cell::Text),
            col("CHANGE", "operationType", 12, Cell::Text),
            col("ON", "resourceId", 38, Cell::Ref),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "RecipientInviteApiResponse",
        cols: &[
            col("NAME", "name", 24, Cell::Text),
            col("EMAIL", "contactEmail", 30, Cell::Text),
            col("STATUS", "status", 12, Cell::Status),
            col("EXPIRES", "expiresAt", 10, Cell::Day),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "RecipientAttachmentWithId",
        cols: &[
            col("FILE", "fileName", 34, Cell::Text),
            col("FORM", "formType", 12, Cell::Text),
            col("UPLOADED", "uploadedAt", 10, Cell::Day),
            col("ID", "id", 38, Cell::Id),
        ],
    },
    View {
        schema: "ApiV1ArAttachmentResponseData",
        cols: &[col("FILE", "fileName", 40, Cell::Text), col("ID", "id", 38, Cell::Id)],
    },
    View {
        schema: "APISafeRequest",
        cols: &[
            col("DATE", "investmentDate", 10, Cell::Day),
            col("INVESTOR", "investor.name", 26, Cell::Text),
            col("AMOUNT", "investmentAmount", 14, Cell::Money),
            col("CAP", "valuationCap", 14, Cell::Money),
            col("ID", "id", 38, Cell::Id),
        ],
    },
];

fn view_for(schema: &str) -> Option<&'static View> {
    VIEWS.iter().find(|view| view.schema == schema)
}

/// Print a reply: raw when asked, a table when it is a list we have a layout
/// for, an aligned detail block for a single object, indented JSON otherwise.
pub fn print(op: &'static Op, value: &Value, raw: &str, as_json: bool) {
    if as_json {
        // Mercury's own bytes, so `| jq` sees exactly what the API said.
        println!("{}", first_non_empty(raw, &value.to_string()));
        return;
    }
    if value.is_null() {
        println!("\n  {}\n", style("Done.").green());
        return;
    }

    match (op.rows(value), view_for(op.item_schema)) {
        (Some(rows), Some(view)) => table(view, rows, op.noun()),
        (Some(rows), None) => println!("{}", pretty(&Value::Array(rows.clone()))),
        (None, _) => detail(value),
    }
}

fn table(view: &View, rows: &[Value], noun: &str) {
    if rows.is_empty() {
        println!("\n  {}\n", style(format!("No {noun}.")).dim());
        return;
    }

    println!();
    let headers: Vec<String> = view.cols.iter().map(|c| pad(c.header, c.width, c.cell)).collect();
    println!("  {}", style(headers.join("  ")).dim());

    for row in rows {
        let cells: Vec<String> = view
            .cols
            .iter()
            .map(|column| {
                let text = render(&at(row, column.field), column.cell);
                let padded = pad(&text, column.width, column.cell);
                paint(&padded, &text, column.cell)
            })
            .collect();
        println!("  {}", cells.join("  "));
    }
    println!("\n  {} {noun}\n", rows.len());
}

/// One object, every field, aligned — a bank record is worth reading in full.
fn detail(value: &Value) {
    let Some(fields) = value.as_object() else {
        println!("{}", pretty(value));
        return;
    };

    let width = fields.keys().map(|key| key.len()).max().unwrap_or(0);
    println!();
    for (key, field) in fields {
        if field.is_null() {
            continue;
        }
        let shown = match field {
            Value::Object(_) | Value::Array(_) => indent(&pretty(field), width + 4),
            _ if is_money(key) => {
                Money::from_json(field).map(|m| m.display()).unwrap_or_else(|| scalar(field))
            }
            _ => scalar(field),
        };
        println!("  {:width$}  {}", style(key).dim(), shown);
    }
    println!();
}

// ── the same layouts, used to choose rather than to read ────────────────────

/// How one row introduces itself in a picker.
///
/// It is the table's own columns minus the id, so a card is offered as
/// "1234  Ada Lovelace  active" — recognisable without being asked to know
/// which of two 36-character uuids is the right one.
pub fn label(schema: &str, row: &Value) -> Option<String> {
    let view = view_for(schema)?;
    let described: Vec<String> = view
        .cols
        .iter()
        .filter(|column| !matches!(column.cell, Cell::Id | Cell::Ref))
        .take(3)
        .map(|column| render(&at(row, column.field), column.cell))
        .filter(|text| text != "—")
        .collect();

    match described.is_empty() {
        true => None,
        false => Some(described.join("  ")),
    }
}

/// Which field holds this resource's id — `id` for most, `cardId` for a card
/// on an account, `requestId` for an approval request.
pub fn id_field(schema: &str) -> Option<&'static str> {
    let view = view_for(schema)?;
    view.cols.iter().find(|column| column.cell == Cell::Id).map(|column| column.field)
}

// ── cells ───────────────────────────────────────────────────────────────────

fn at(row: &Value, path: &str) -> Value {
    let mut current = row;
    for step in path.split('.') {
        current = &current[step];
    }
    current.clone()
}

fn render(value: &Value, cell: Cell) -> String {
    if value.is_null() {
        return "—".into();
    }
    match cell {
        Cell::Money => Money::from_json(value).map(|m| m.display()).unwrap_or_else(|| scalar(value)),
        Cell::Day => scalar(value).chars().take(10).collect(),
        _ => scalar(value),
    }
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "—".into(),
        other => other.to_string(),
    }
}

/// Money reads right-aligned so the decimal points line up; everything else left.
fn pad(text: &str, width: usize, cell: Cell) -> String {
    let text = truncate(text, width);
    let gap = width.saturating_sub(text.chars().count());
    match cell {
        Cell::Money => format!("{}{}", " ".repeat(gap), text),
        _ => format!("{}{}", text, " ".repeat(gap)),
    }
}

fn truncate(text: &str, width: usize) -> String {
    match text.chars().count() > width {
        true => format!("{}…", text.chars().take(width.saturating_sub(1)).collect::<String>()),
        false => text.to_string(),
    }
}

fn paint(padded: &str, value: &str, cell: Cell) -> String {
    match cell {
        Cell::Id | Cell::Ref => style(padded).dim().to_string(),
        Cell::Status => status_colour(padded, value),
        Cell::Money if value.starts_with('-') => style(padded).red().to_string(),
        _ => padded.to_string(),
    }
}

/// Colour carries meaning here: money that failed should not look like money
/// that arrived.
fn status_colour(padded: &str, value: &str) -> String {
    match value.trim() {
        "sent" | "posted" | "active" | "paid" | "approved" | "completed" => style(padded).green().to_string(),
        "failed" | "cancelled" | "canceled" | "disabled" | "expired" | "rejected" => {
            style(padded).red().to_string()
        }
        "pending" | "frozen" | "paused" | "processing" | "draft" | "open" => {
            style(padded).yellow().to_string()
        }
        _ => padded.to_string(),
    }
}

fn is_money(key: &str) -> bool {
    let key = key.to_lowercase();
    ["amount", "balance", "limit", "returns", "cap"].iter().any(|word| key.contains(word))
}

pub fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn indent(text: &str, spaces: usize) -> String {
    text.replace('\n', &format!("\n{}", " ".repeat(spaces)))
}

fn first_non_empty<'a>(first: &'a str, second: &'a str) -> &'a str {
    match first.trim().is_empty() {
        true => second,
        false => first,
    }
}

pub fn success(message: &str) {
    println!("\n  {} {message}", style("✓").green().bold());
}

pub fn warn(message: &str) {
    println!("  {} {message}", style("!").yellow().bold());
}

pub fn spinner(message: &str) -> indicatif::ProgressBar {
    let bar = indicatif::ProgressBar::new_spinner();
    bar.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("  {spinner:.cyan} {msg}")
            .unwrap(),
    );
    bar.set_message(message.to_string());
    bar.enable_steady_tick(std::time::Duration::from_millis(80));
    bar
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_layout_names_a_schema_that_exists() {
        for view in VIEWS {
            let used = crate::ops::OPS.iter().any(|op| op.item_schema == view.schema);
            assert!(used, "no operation returns a {}", view.schema);
        }
    }

    #[test]
    fn the_resources_worth_reading_have_a_layout() {
        for schema in ["Account", "Transaction", "Card", "RecipientInfo", "TreasuryAccount"] {
            assert!(view_for(schema).is_some(), "{schema} has no layout");
        }
    }

    #[test]
    fn nested_fields_are_reachable_and_missing_ones_are_blank() {
        let row = json!({"investor": {"name": "Ada"}, "id": "s_1"});
        assert_eq!(at(&row, "investor.name"), json!("Ada"));
        assert!(at(&row, "investor.address.city").is_null());
        assert_eq!(render(&at(&row, "nope"), Cell::Text), "—");
    }

    #[test]
    fn money_is_right_aligned_and_dates_are_cut_to_the_day() {
        assert_eq!(pad(&render(&json!(1234.5), Cell::Money), 12, Cell::Money), "   $1,234.50");
        assert_eq!(render(&json!("2026-07-08T00:00:00Z"), Cell::Day), "2026-07-08");
    }

    #[test]
    fn long_values_are_cut_rather_than_wrapped() {
        assert_eq!(pad("a-very-long-counterparty-name", 10, Cell::Text), "a-very-lo…");
        assert_eq!(pad("short", 8, Cell::Text), "short   ");
    }

    #[test]
    fn amount_keys_are_recognised_in_the_detail_view() {
        assert!(is_money("currentBalance") && is_money("amount") && is_money("valuationCap"));
        assert!(!is_money("status") && !is_money("counterpartyName"));
    }
}
