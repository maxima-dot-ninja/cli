use anyhow::Result;
use console::style;
use serde_json::Value;

/// A column or a field: the heading it prints under, and the JSON pointer its value comes from.
pub type Column = (&'static str, &'static str);

pub fn header() {
    println!();
    println!("  {}  {}", style("hku").magenta().bold(), style("Heroku from the terminal").dim());
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
        println!("  {}", line.join("  ").trim_end());
    }
    println!();
    println!("  {}", style(format!("{} row{}", rows.len(), if rows.len() == 1 { "" } else { "s" })).dim());
    println!();
}

/// A list from the API as a table, one column per pointer, or the raw list with `--json`.
pub fn list(items: &[Value], columns: &[Column], as_json: bool) -> Result<()> {
    if as_json {
        return json(&Value::Array(items.to_vec()));
    }
    let heads: Vec<String> = columns.iter().map(|(head, _)| head.to_string()).collect();
    let rows: Vec<Vec<Value>> = items
        .iter()
        .map(|item| columns.iter().map(|(_, at)| item.pointer(at).cloned().unwrap_or(Value::Null)).collect())
        .collect();
    table(&heads, &rows, 60);
    Ok(())
}

/// One item from the API as labelled lines, or the raw item with `--json`.
pub fn fields(item: &Value, fields: &[Column], as_json: bool) -> Result<()> {
    if as_json {
        return json(item);
    }
    let width = fields.iter().map(|(label, _)| label.chars().count()).max().unwrap_or(0);
    println!();
    for (label, at) in fields {
        let value = cell(item.pointer(at).unwrap_or(&Value::Null), 100);
        println!("  {:<width$}  {}", style(label).dim(), value);
    }
    println!();
    Ok(())
}

pub fn note(text: &str) {
    println!("  {}", style(text).dim());
}
