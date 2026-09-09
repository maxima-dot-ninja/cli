use crate::client::QueryResult;
use anyhow::Result;
use console::style;
use serde_json::Value;

pub fn header() {
    println!();
    println!("  {}  {}", style("phog").cyan().bold(), style("PostHog from the terminal").dim());
    println!();
}

pub fn json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// A cell as one line: strings bare, everything else compact JSON, long ones cut.
pub fn cell(value: &Value, width: usize) -> String {
    let text = match value {
        Value::Null => "—".to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let text = text.replace('\n', " ");
    match text.chars().count() > width {
        true => format!("{}…", text.chars().take(width.saturating_sub(1)).collect::<String>()),
        false => text,
    }
}

/// Columns sized to their content, capped so a JSON blob does not eat the terminal.
pub fn table(columns: &[String], rows: &[Vec<Value>], max_width: usize) {
    if rows.is_empty() {
        println!("\n  {}\n", style("No rows.").dim());
        return;
    }
    let count = columns.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    let mut widths = vec![0usize; count];
    for (i, column) in columns.iter().enumerate() {
        widths[i] = column.chars().count();
    }
    for row in rows {
        for (i, value) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell(value, max_width).chars().count());
        }
    }
    for width in widths.iter_mut() {
        *width = (*width).min(max_width);
    }

    println!();
    let head: Vec<String> = (0..count)
        .map(|i| format!("{:<w$}", columns.get(i).cloned().unwrap_or_default(), w = widths[i]))
        .collect();
    println!("  {}", style(head.join("  ")).dim());
    for row in rows {
        let line: Vec<String> = (0..count)
            .map(|i| format!("{:<w$}", cell(row.get(i).unwrap_or(&Value::Null), max_width), w = widths[i]))
            .collect();
        println!("  {}", line.join("  "));
    }
    println!();
    println!("  {}", style(format!("{} row{}", rows.len(), if rows.len() == 1 { "" } else { "s" })).dim());
    println!();
}

pub fn query_result(result: &QueryResult, as_json: bool) -> Result<()> {
    if as_json {
        let rows: Vec<Value> = result
            .rows
            .iter()
            .map(|row| {
                let mut object = serde_json::Map::new();
                for (i, value) in row.iter().enumerate() {
                    object.insert(
                        result.columns.get(i).cloned().unwrap_or_else(|| format!("column_{i}")),
                        value.clone(),
                    );
                }
                Value::Object(object)
            })
            .collect();
        return json(&Value::Array(rows));
    }
    table(&result.columns, &result.rows, 60);
    Ok(())
}

pub fn dashboards(items: &[Value], as_json: bool) -> Result<()> {
    if as_json {
        return json(&Value::Array(items.to_vec()));
    }
    if items.is_empty() {
        println!("\n  {}\n", style("No dashboards.").dim());
        return Ok(());
    }
    println!();
    println!(
        "  {:<7} {:<40} {:<8} {}",
        style("ID").dim(),
        style("NAME").dim(),
        style("TILES").dim(),
        style("TAGS").dim()
    );
    for item in items {
        let id = item.get("id").and_then(Value::as_u64).unwrap_or_default();
        let name = cell(item.get("name").unwrap_or(&Value::Null), 40);
        let tiles = item.get("tiles").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        let tags = item
            .get("tags")
            .and_then(Value::as_array)
            .map(|t| t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        let managed = tags.split(", ").any(|t| t.starts_with("phog:"));
        println!(
            "  {:<7} {:<40} {:<8} {}",
            id,
            name,
            tiles,
            if managed { style(tags).cyan().to_string() } else { style(tags).dim().to_string() }
        );
    }
    println!();
    Ok(())
}

pub fn note(text: &str) {
    println!("  {}", style(text).dim());
}
