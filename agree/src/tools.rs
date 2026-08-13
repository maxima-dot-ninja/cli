use crate::client::Client;
use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    Get,
    Post,
    Patch,
    Delete,
}

/// One API operation the model is allowed to invoke.
///
/// Kept as data rather than code: every tool is the same generic HTTP call with a
/// different path, so adding an endpoint is a row in the table, not a new branch.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    /// Argument names, for the prompt. `{id}` style names fill path placeholders;
    /// the rest become query parameters or body fields.
    pub args: &'static str,
    pub method: Method,
    pub path: &'static str,
    /// Body wrapper the API expects, e.g. `invoice` for {"invoice": {...}}
    pub body_key: Option<&'static str>,
    /// Anything that changes state must be confirmed before it runs
    pub mutates: bool,
}

const fn read(
    name: &'static str,
    description: &'static str,
    args: &'static str,
    path: &'static str,
) -> Tool {
    Tool { name, description, args, method: Method::Get, path, body_key: None, mutates: false }
}

pub const TOOLS: &[Tool] = &[
    // ── Invoices ────────────────────────────────────────────────────────────
    // Handled by the interactive form rather than a plain HTTP call: it resolves
    // the payee, converts dollars to cents and pins the repeat to a weekday.
    Tool {
        name: "create_invoice",
        description: "Create an invoice, one-off or recurring. Opens a short form so the user can confirm the payee, amount and dates.",
        args: "amount (as the user said it, e.g. \"5000\" or \"$1.5k\"), payee (name as spoken), repeat_unit (week|month), repeat_frequency, due_date (YYYY-MM-DD), memo",
        method: Method::Post,
        path: "/api/v1/invoices",
        body_key: Some("invoice"),
        mutates: true,
    },
    read(
        "list_invoices",
        "List invoices. Filter by status, customer name, amount or date range.",
        "statuses (created|due|sent|canceled|paid|failed|refunded|draft, comma separated), customer, date_start, date_end, date_type (paid_at|due_at|scheduled_at), amount_min, amount_max, page_size",
        "/api/v1/invoices",
    ),
    read("get_invoice", "Full detail for one invoice.", "id", "/api/v1/invoices/{id}"),
    Tool {
        name: "update_invoice",
        description: "Change fields on an existing invoice.",
        args: "id, plus any of: memo, due_at, scheduled_at, amount",
        method: Method::Patch,
        path: "/api/v1/invoices/{id}",
        body_key: Some("invoice"),
        mutates: true,
    },
    Tool {
        name: "delete_invoice",
        description: "Permanently delete an invoice.",
        args: "id",
        method: Method::Delete,
        path: "/api/v1/invoices/{id}",
        body_key: None,
        mutates: true,
    },
    Tool {
        name: "send_invoice",
        description: "Email an existing invoice to its billing contact now.",
        args: "id",
        method: Method::Post,
        path: "/api/v1/invoices/{id}/send",
        body_key: None,
        mutates: true,
    },
    Tool {
        name: "mark_invoice_paid",
        description: "Mark an invoice as paid without taking payment.",
        args: "id",
        method: Method::Post,
        path: "/api/v1/invoices/{id}/mark_as_paid",
        body_key: None,
        mutates: true,
    },
    Tool {
        name: "mark_invoice_sent",
        description: "Mark an invoice as sent without emailing it.",
        args: "id",
        method: Method::Post,
        path: "/api/v1/invoices/{id}/mark_as_sent",
        body_key: None,
        mutates: true,
    },
    read("invoice_pdf", "Get a download link for an invoice PDF.", "id", "/api/v1/invoices/{id}/pdf"),
    read(
        "invoice_receipt_pdf",
        "Get a download link for a paid invoice's receipt.",
        "id",
        "/api/v1/invoices/{id}/receipt_pdf",
    ),
    // ── Contacts ────────────────────────────────────────────────────────────
    read(
        "list_contacts",
        "List contacts. NOTE: the API cannot filter by person name — only email and company. To find someone by first name, list them all and read the names yourself.",
        "email, company, page_size",
        "/api/v1/contacts",
    ),
    read("get_contact", "Full detail for one contact.", "id", "/api/v1/contacts/{id}"),
    Tool {
        name: "create_contact",
        description: "Create a contact. Never invent an email address — ask the user.",
        args: "name, email, company, title, address",
        method: Method::Post,
        path: "/api/v1/contacts",
        body_key: Some("contact"),
        mutates: true,
    },
    Tool {
        name: "update_contact",
        description: "Change fields on a contact.",
        args: "id, plus any of: name, email, company, title, address",
        method: Method::Patch,
        path: "/api/v1/contacts/{id}",
        body_key: Some("contact"),
        mutates: true,
    },
    Tool {
        name: "delete_contact",
        description: "Permanently delete a contact.",
        args: "id",
        method: Method::Delete,
        path: "/api/v1/contacts/{id}",
        body_key: None,
        mutates: true,
    },
    // ── Customers ───────────────────────────────────────────────────────────
    read("list_customers", "List customers (business entities).", "name, page_size", "/api/v1/customers"),
    read("get_customer", "Full detail for one customer.", "id", "/api/v1/customers/{id}"),
    Tool {
        name: "create_customer",
        description: "Create a customer.",
        args: "name, business_type (company|individual|non_profit|government_entity), contact_id",
        method: Method::Post,
        path: "/api/v1/customers",
        body_key: Some("customer"),
        mutates: true,
    },
    Tool {
        name: "update_customer",
        description: "Change fields on a customer.",
        args: "id, plus any of: name, business_type, primary_contact_id",
        method: Method::Patch,
        path: "/api/v1/customers/{id}",
        body_key: Some("customer"),
        mutates: true,
    },
    // ── Agreements ──────────────────────────────────────────────────────────
    read("list_agreements", "List agreements.", "page_size", "/api/v1/agreements"),
    read("get_agreement", "Full detail for one agreement.", "id", "/api/v1/agreements/{id}"),
    read(
        "list_templates",
        "List agreement templates. An agreement can only be created from one of these.",
        "",
        "/api/v1/agreements/templates",
    ),
    read("get_template", "Full detail for one template.", "id", "/api/v1/agreements/templates/{id}"),
    Tool {
        name: "update_agreement",
        description: "Change fields on an agreement.",
        args: "id, plus any of: name, starts_at, ends_at",
        method: Method::Patch,
        path: "/api/v1/agreements/{id}",
        body_key: Some("agreement"),
        mutates: true,
    },
    Tool {
        name: "delete_agreement",
        description: "Permanently delete an agreement.",
        args: "id",
        method: Method::Delete,
        path: "/api/v1/agreements/{id}",
        body_key: None,
        mutates: true,
    },
    Tool {
        name: "send_agreement",
        description: "Send an agreement for signature.",
        args: "id, delivery_mode (embedded|managed), message",
        method: Method::Post,
        path: "/api/v1/agreements/{id}/send",
        body_key: None,
        mutates: true,
    },
    read("agreement_pdf", "Get a download link for an agreement PDF.", "id", "/api/v1/agreements/{id}/pdf"),
    // ── Webhooks ────────────────────────────────────────────────────────────
    read("list_webhooks", "List webhook endpoints.", "", "/api/v1/webhooks"),
    Tool {
        name: "create_webhook",
        description: "Register a webhook endpoint.",
        args: "url, events (list of event names)",
        method: Method::Post,
        path: "/api/v1/webhooks",
        body_key: Some("webhook_endpoint"),
        mutates: true,
    },
    Tool {
        name: "delete_webhook",
        description: "Delete a webhook endpoint.",
        args: "id",
        method: Method::Delete,
        path: "/api/v1/webhooks/{id}",
        body_key: None,
        mutates: true,
    },
    // ── Reports ─────────────────────────────────────────────────────────────
    read("revenue_stats", "Revenue summary statistics.", "", "/api/v1/reports/revenue/stats"),
    read("revenue_by_customer", "Customers ranked by revenue.", "", "/api/v1/reports/revenue/customers"),
    read("revenue_by_mrr", "Customers ranked by monthly recurring revenue.", "", "/api/v1/reports/revenue/customers_by_mrr"),
    read("cashflow_stats", "Cashflow summary statistics.", "", "/api/v1/reports/cashflow/stats"),
    read("cashflow_forecast", "Forecast of expected cash.", "", "/api/v1/reports/cashflow/forecast"),
    read("outstanding_invoices", "Invoices still awaiting payment.", "", "/api/v1/reports/cashflow/outstanding_invoices"),
    read("aging_invoices", "Invoices grouped by how overdue they are.", "", "/api/v1/reports/recovery/aging/invoices"),
    read("stalled_invoices", "Invoices stuck in a stage.", "", "/api/v1/reports/recovery/leakage/stalled_invoices"),
];

pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

/// The tool list as the model sees it.
pub fn catalogue() -> String {
    TOOLS
        .iter()
        .map(|tool| {
            let mark = if tool.mutates { " [CHANGES DATA]" } else { "" };
            match tool.args.is_empty() {
                true => format!("- {}{}: {}", tool.name, mark, tool.description),
                false => format!(
                    "- {}{}: {}\n    args: {}",
                    tool.name, mark, tool.description, tool.args
                ),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Fill `{placeholders}` from args, returning the path and the leftover args.
fn split_path(tool: &Tool, args: &Map<String, Value>) -> Result<(String, Map<String, Value>)> {
    let mut path = tool.path.to_string();
    let mut rest = args.clone();

    while let Some(start) = path.find('{') {
        let Some(end) = path[start..].find('}').map(|i| start + i) else {
            bail!("Malformed path template: {}", tool.path);
        };
        let key = path[start + 1..end].to_string();
        let value = rest
            .remove(&key)
            .map(|v| match v {
                Value::String(s) => s,
                other => other.to_string().trim_matches('"').to_string(),
            })
            .unwrap_or_default();

        if value.is_empty() {
            bail!("{} needs a `{}`", tool.name, key);
        }
        path.replace_range(start..=end, &value);
    }

    Ok((path, rest))
}

/// Human-readable summary of what a call will do, for the confirmation prompt.
pub fn describe_call(tool: &Tool, args: &Map<String, Value>) -> String {
    let (path, rest) = split_path(tool, args).unwrap_or_else(|_| (tool.path.into(), args.clone()));
    let method = match tool.method {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Patch => "PATCH",
        Method::Delete => "DELETE",
    };
    match rest.is_empty() {
        true => format!("{method} {path}"),
        false => format!(
            "{method} {path}\n  {}",
            serde_json::to_string_pretty(&Value::Object(rest)).unwrap_or_default().replace('\n', "\n  ")
        ),
    }
}

pub async fn run(api: &Client, tool: &Tool, args: &Map<String, Value>) -> Result<Value> {
    let (path, rest) = split_path(tool, args)?;

    match tool.method {
        Method::Get => {
            let query: Vec<(String, String)> = rest
                .iter()
                .map(|(k, v)| {
                    let text = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    (k.clone(), text)
                })
                .collect();
            api.get_raw(&path, &query).await
        }
        Method::Delete => api.delete_raw(&path).await,
        Method::Post | Method::Patch => {
            let body = match tool.body_key {
                Some(key) => json!({ key: Value::Object(rest) }),
                None => Value::Object(rest),
            };
            match tool.method {
                Method::Post => api.post_raw(&path, &body).await,
                _ => api.patch_raw(&path, &body).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn fills_path_placeholders_and_keeps_the_rest() {
        let tool = find("update_invoice").unwrap();
        let (path, rest) = split_path(tool, &args(json!({"id": "abc", "memo": "hi"}))).unwrap();
        assert_eq!(path, "/api/v1/invoices/abc");
        assert_eq!(rest.len(), 1);
        assert_eq!(rest["memo"], "hi");
    }

    #[test]
    fn a_missing_id_is_an_error_not_a_broken_url() {
        let tool = find("delete_invoice").unwrap();
        let err = split_path(tool, &args(json!({}))).unwrap_err().to_string();
        assert!(err.contains("needs a `id`"), "got: {err}");
    }

    #[test]
    fn every_destructive_tool_is_marked() {
        for tool in TOOLS {
            let destructive = tool.name.starts_with("delete_")
                || tool.name.starts_with("create_")
                || tool.name.starts_with("update_")
                || tool.name.starts_with("send_")
                || tool.name.starts_with("mark_");
            if destructive {
                assert!(tool.mutates, "{} changes data but is not marked", tool.name);
            }
            // and the inverse: nothing marked as mutating should be a GET
            if tool.mutates {
                assert!(tool.method != Method::Get, "{} mutates but is a GET", tool.name);
            }
        }
    }

    #[test]
    fn catalogue_flags_mutations_for_the_model() {
        let text = catalogue();
        assert!(text.contains("delete_invoice [CHANGES DATA]"));
        assert!(text.contains("list_invoices:"));
        assert!(!text.contains("list_invoices [CHANGES DATA]"));
    }

    #[test]
    fn tool_names_are_unique() {
        let mut names: Vec<&str> = TOOLS.iter().map(|t| t.name).collect();
        names.sort();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate tool name");
    }
}
