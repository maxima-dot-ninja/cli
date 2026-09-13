//! Monthly counts per event, for the journey map on the website's admin.
//!
//! Two HogQL queries: every named event by month, and `$pageview` by month for
//! each route pattern you pass. The answer is one JSON document with the months
//! in order and one array of counts per event or path, aligned to those months,
//! so a page can index a month without doing any date maths of its own.

use crate::client::{self, Client};
use anyhow::{Context, Result};
use console::style;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Counts {
    pub generated_at: String,
    pub months: Vec<String>,
    pub events: BTreeMap<String, Vec<u64>>,
    pub pageviews: BTreeMap<String, Vec<u64>>,
}

pub fn collect(client: &Client, months: usize, paths: &[String]) -> Result<Counts> {
    let months = months.max(1);
    let labels = month_labels(months);
    let since = format!("toStartOfMonth(now() - INTERVAL {} MONTH)", months - 1);

    let events = client.query(&format!(
        "SELECT event, formatDateTime(toStartOfMonth(timestamp), '%Y-%m') AS month, count() AS n \
         FROM events WHERE timestamp >= {since} AND event NOT LIKE '$%' \
         GROUP BY event, month ORDER BY event, month LIMIT 100000"
    ))?;
    let mut by_event: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for row in &events.rows {
        let name = row.first().and_then(Value::as_str).unwrap_or_default().to_string();
        let month = row.get(1).and_then(Value::as_str).unwrap_or_default();
        let n = row.get(2).and_then(Value::as_u64).unwrap_or(0);
        let Some(index) = labels.iter().position(|m| m == month) else { continue };
        by_event.entry(name).or_insert_with(|| vec![0; months])[index] = n;
    }

    let mut by_path: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    if !paths.is_empty() {
        let columns: Vec<String> = paths
            .iter()
            .enumerate()
            .map(|(i, path)| {
                format!("countIf(match(properties.$pathname, {})) AS p{i}", client::literal(&regex_of(path)))
            })
            .collect();
        let pageviews = client.query(&format!(
            "SELECT formatDateTime(toStartOfMonth(timestamp), '%Y-%m') AS month, {} \
             FROM events WHERE event = '$pageview' AND timestamp >= {since} \
             GROUP BY month ORDER BY month",
            columns.join(", ")
        ))?;
        for path in paths {
            by_path.insert(path.clone(), vec![0; months]);
        }
        for row in &pageviews.rows {
            let month = row.first().and_then(Value::as_str).unwrap_or_default();
            let Some(index) = labels.iter().position(|m| m == month) else { continue };
            for (i, path) in paths.iter().enumerate() {
                let n = row.get(i + 1).and_then(Value::as_u64).unwrap_or(0);
                by_path.get_mut(path).unwrap()[index] = n;
            }
        }
    }

    Ok(Counts { generated_at: now_iso(), months: labels, events: by_event, pageviews: by_path })
}

pub fn to_json(counts: &Counts) -> Value {
    json!({
        "generated_at": counts.generated_at,
        "months": counts.months,
        "events": counts.events,
        "pageviews": counts.pageviews,
    })
}

/// Write the counts, or print them when there is no file. Rerunning overwrites
/// the same file with the same shape, so the page that imports it never sees a
/// half-written document: the write goes to a sibling first, then renames.
pub fn run(client: &Client, months: usize, paths: Vec<String>, out: Option<String>) -> Result<()> {
    let counts = collect(client, months, &paths)?;
    let text = serde_json::to_string_pretty(&to_json(&counts))? + "\n";
    let Some(path) = out else {
        print!("{text}");
        return Ok(());
    };
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let staging = format!("{path}.tmp");
    std::fs::write(&staging, &text).with_context(|| format!("Could not write {staging}"))?;
    std::fs::rename(&staging, &path).with_context(|| format!("Could not move {staging} to {path}"))?;
    let first = counts.months.first().cloned().unwrap_or_default();
    let last = counts.months.last().cloned().unwrap_or_default();
    println!(
        "\n  {} wrote {} ({} events, {} paths, {} to {})\n",
        style("✓").green(),
        path,
        counts.events.len(),
        counts.pageviews.len(),
        first,
        last
    );
    Ok(())
}

/// A route pattern as a regex: `:param` segments match one path segment, an
/// optional trailing slash is tolerated, and everything else is literal.
pub fn regex_of(pattern: &str) -> String {
    let body: Vec<String> = pattern
        .trim_end_matches('/')
        .split('/')
        .map(|segment| match segment.starts_with(':') {
            true => "[^/]+".to_string(),
            false => regex_escape(segment),
        })
        .collect();
    let body = body.join("/");
    match body.is_empty() {
        true => "^/?$".to_string(),
        false => format!("^{body}/?$"),
    }
}

fn regex_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.+*?()[]{}|^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The last `count` months as `YYYY-MM`, oldest first, ending on the current month in UTC.
fn month_labels(count: usize) -> Vec<String> {
    let (year, month, _) = civil_from_days(days_since_epoch());
    let mut serial = year * 12 + (month as i64 - 1);
    let mut labels = Vec::with_capacity(count);
    for _ in 0..count {
        labels.push(format!("{:04}-{:02}", serial.div_euclid(12), serial.rem_euclid(12) + 1));
        serial -= 1;
    }
    labels.reverse();
    labels
}

fn now_iso() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

fn days_since_epoch() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0).div_euclid(86_400)
}

/// Days since 1970-01-01 to a proleptic Gregorian date. Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_become_anchored_regexes() {
        assert_eq!(regex_of("/"), "^/?$");
        assert_eq!(regex_of("/pricing"), "^/pricing/?$");
        assert_eq!(regex_of("/careers/:id"), "^/careers/[^/]+/?$");
        assert_eq!(
            regex_of("/coworking-spaces/:regionSlug/:neighborhoodSlug/:placeSlug"),
            "^/coworking-spaces/[^/]+/[^/]+/[^/]+/?$"
        );
        assert_eq!(regex_of("/a.b"), "^/a\\.b/?$");
    }

    #[test]
    fn civil_dates_round_trip_known_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_697), (2026, 9, 1));
    }

    #[test]
    fn month_labels_are_ordered_and_sized() {
        let labels = month_labels(12);
        assert_eq!(labels.len(), 12);
        assert!(labels.windows(2).all(|w| w[0] < w[1]));
    }
}
