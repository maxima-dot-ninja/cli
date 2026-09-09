//! A dashboard as a file.
//!
//! The file is the truth: `phog dashboards apply` makes PostHog match it and
//! nothing else, and running it twice changes nothing the second time. Every
//! insight carries a `key`, and the key becomes a tag on the PostHog insight
//! (`phog:<dashboard slug>:<key>`), which is how the next apply finds the one
//! it made last time instead of making another.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// The tag that marks anything phog manages.
pub const TAG: &str = "phog";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub name: String,
    /// Stable handle. The dashboard is found by the tag `phog:<slug>`, so
    /// renaming the dashboard does not orphan it.
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Default date range for every insight that does not set its own, e.g. `-30d`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_from: Option<String>,
    #[serde(default)]
    pub insights: Vec<Insight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub key: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: Kind,
    /// trends, number and funnel: one entry per line or step
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<Series>,
    /// hogql: the SQL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// raw: a complete PostHog query node, for anything the shorthands do not cover
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
    /// hour | day | week | month
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_from: Option<String>,
    /// ActionsLineGraph | ActionsBar | ActionsBarValue | ActionsPie | ActionsTable | BoldNumber | WorldMap
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// An event property to break the series down by
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakdown: Option<String>,
    /// funnel: how long a person has to finish, e.g. `14d`, `2h`, `30m`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    /// Filters on the whole insight
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<Filter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Trends,
    Number,
    Funnel,
    Hogql,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// total | dau | weekly_active | monthly_active | unique_session | sum | avg | min | max | median | p90 | p95 | p99
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub math: Option<String>,
    /// The event property the math runs over, for sum/avg/min/max/median/p*
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<Filter>,
}

/// A property filter. `key` is the property; `type` is event (default), person,
/// or hogql, in which case `key` is the whole expression and `value` is ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// exact | is_not | icontains | not_icontains | regex | gt | lt | is_set | is_not_set
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Where the tile sits, on PostHog's 12-column grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub fn parse(text: &str) -> Result<Dashboard> {
    let dashboard: Dashboard = serde_yaml::from_str(text).context("The dashboard file did not parse")?;
    dashboard.validate()?;
    Ok(dashboard)
}

impl Dashboard {
    pub fn tag(&self) -> String {
        format!("{TAG}:{}", self.slug)
    }

    /// Every tag the dashboard carries: the user's, plus the one that marks it as managed.
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags = vec![TAG.to_string(), self.tag()];
        tags.extend(self.tags.iter().cloned());
        dedupe(tags)
    }

    fn validate(&self) -> Result<()> {
        if self.slug.trim().is_empty()
            || !self.slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("`slug` must be letters, digits, dashes or underscores, got {:?}", self.slug);
        }
        let mut seen = std::collections::HashSet::new();
        for insight in &self.insights {
            if !seen.insert(&insight.key) {
                bail!("insight key {:?} appears twice", insight.key);
            }
            insight.validate(self)?;
        }
        Ok(())
    }
}

impl Insight {
    pub fn tag(&self, dashboard: &Dashboard) -> String {
        format!("{TAG}:{}:{}", dashboard.slug, self.key)
    }

    pub fn all_tags(&self, dashboard: &Dashboard) -> Vec<String> {
        let mut tags = vec![TAG.to_string(), self.tag(dashboard)];
        tags.extend(dashboard.tags.iter().cloned());
        dedupe(tags)
    }

    fn validate(&self, dashboard: &Dashboard) -> Result<()> {
        if self.key.trim().is_empty()
            || !self.key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("insight key must be letters, digits, dashes or underscores, got {:?}", self.key);
        }
        match self.kind {
            Kind::Trends | Kind::Number if self.series.is_empty() => {
                bail!("{}: a {:?} insight needs at least one series", self.key, self.kind)
            }
            Kind::Funnel if self.series.len() < 2 => bail!("{}: a funnel needs at least two steps", self.key),
            Kind::Hogql if self.query.as_deref().map(str::trim).unwrap_or_default().is_empty() => {
                bail!("{}: a hogql insight needs `query`", self.key)
            }
            Kind::Raw if self.raw.is_none() => {
                bail!("{}: a raw insight needs `raw`, a complete PostHog query node", self.key)
            }
            _ => {}
        }
        if let Some(window) = &self.window {
            parse_window(window).with_context(|| format!("{} in dashboard {}", self.key, dashboard.slug))?;
        }
        Ok(())
    }

    /// The query node PostHog stores on the insight. This is the whole translation
    /// from the file's shorthand to what the API wants.
    pub fn to_query(&self, dashboard: &Dashboard) -> Result<Value> {
        let date_from = self
            .date_from
            .clone()
            .or_else(|| dashboard.date_from.clone())
            .unwrap_or_else(|| "-30d".to_string());
        let properties: Vec<Value> = self.filters.iter().map(Filter::to_value).collect();

        Ok(match self.kind {
            Kind::Raw => self.raw.clone().unwrap_or(Value::Null),
            Kind::Hogql => json!({
                "kind": "DataTableNode",
                "full": true,
                "source": { "kind": "HogQLQuery", "query": self.query.clone().unwrap_or_default() }
            }),
            Kind::Trends | Kind::Number => {
                let display = self.display.clone().unwrap_or_else(|| match self.kind {
                    Kind::Number => "BoldNumber".to_string(),
                    _ => "ActionsLineGraph".to_string(),
                });
                let mut source = Map::new();
                source.insert("kind".into(), json!("TrendsQuery"));
                source.insert(
                    "series".into(),
                    Value::Array(self.series.iter().map(Series::to_value).collect()),
                );
                source.insert(
                    "interval".into(),
                    json!(self.interval.clone().unwrap_or_else(|| "day".to_string())),
                );
                source.insert("dateRange".into(), json!({ "date_from": date_from }));
                source.insert("trendsFilter".into(), json!({ "display": display }));
                if let Some(breakdown) = &self.breakdown {
                    source.insert(
                        "breakdownFilter".into(),
                        json!({ "breakdown": breakdown, "breakdown_type": "event" }),
                    );
                }
                if !properties.is_empty() {
                    source.insert("properties".into(), Value::Array(properties));
                }
                json!({ "kind": "InsightVizNode", "source": Value::Object(source) })
            }
            Kind::Funnel => {
                let (interval, unit) = parse_window(self.window.as_deref().unwrap_or("14d"))?;
                let mut source = Map::new();
                source.insert("kind".into(), json!("FunnelsQuery"));
                source.insert(
                    "series".into(),
                    Value::Array(self.series.iter().map(Series::to_value).collect()),
                );
                source.insert("dateRange".into(), json!({ "date_from": date_from }));
                source.insert(
                    "funnelsFilter".into(),
                    json!({ "funnelVizType": "steps", "funnelWindowInterval": interval, "funnelWindowIntervalUnit": unit }),
                );
                if let Some(breakdown) = &self.breakdown {
                    source.insert(
                        "breakdownFilter".into(),
                        json!({ "breakdown": breakdown, "breakdown_type": "event" }),
                    );
                }
                if !properties.is_empty() {
                    source.insert("properties".into(), Value::Array(properties));
                }
                json!({ "kind": "InsightVizNode", "source": Value::Object(source) })
            }
        })
    }

    /// The best reading of a query node PostHog handed back, for `export`. Anything
    /// the shorthands cannot express comes back as `raw`, which round-trips exactly.
    pub fn from_query(key: &str, name: &str, description: Option<String>, query: &Value) -> Insight {
        let mut insight = Insight {
            key: key.to_string(),
            name: name.to_string(),
            description,
            kind: Kind::Raw,
            series: vec![],
            query: None,
            raw: Some(query.clone()),
            interval: None,
            date_from: None,
            display: None,
            breakdown: None,
            window: None,
            filters: vec![],
            layout: None,
        };
        let source = match query.get("source") {
            Some(source) => source,
            None => return insight,
        };
        let source_kind = source.get("kind").and_then(Value::as_str).unwrap_or_default();
        let date_from = source.pointer("/dateRange/date_from").and_then(Value::as_str).map(str::to_string);
        let breakdown =
            source.pointer("/breakdownFilter/breakdown").and_then(Value::as_str).map(str::to_string);
        let filters = source
            .get("properties")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Filter::from_value).collect())
            .unwrap_or_default();
        let series: Option<Vec<Series>> = source
            .get("series")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Series::from_value).collect());

        match source_kind {
            "HogQLQuery" => {
                insight.kind = Kind::Hogql;
                insight.query = source.get("query").and_then(Value::as_str).map(str::to_string);
                insight.raw = None;
            }
            "TrendsQuery" if series.is_some() => {
                let display =
                    source.pointer("/trendsFilter/display").and_then(Value::as_str).map(str::to_string);
                insight.kind =
                    if display.as_deref() == Some("BoldNumber") { Kind::Number } else { Kind::Trends };
                insight.display = display.filter(|d| d != "BoldNumber" && d != "ActionsLineGraph");
                insight.series = series.unwrap_or_default();
                insight.interval =
                    source.get("interval").and_then(Value::as_str).map(str::to_string).filter(|i| i != "day");
                insight.date_from = date_from;
                insight.breakdown = breakdown;
                insight.filters = filters;
                insight.raw = None;
            }
            "FunnelsQuery" if series.is_some() => {
                insight.kind = Kind::Funnel;
                insight.series = series.unwrap_or_default();
                insight.date_from = date_from;
                insight.breakdown = breakdown;
                insight.filters = filters;
                let interval = source.pointer("/funnelsFilter/funnelWindowInterval").and_then(Value::as_u64);
                let unit = source.pointer("/funnelsFilter/funnelWindowIntervalUnit").and_then(Value::as_str);
                insight.window = match (interval, unit) {
                    (Some(n), Some(unit)) => Some(format!("{n}{}", unit.chars().next().unwrap_or('d'))),
                    _ => None,
                };
                insight.raw = None;
            }
            _ => {}
        }
        insight
    }
}

impl Series {
    fn to_value(&self) -> Value {
        let mut node = Map::new();
        node.insert("kind".into(), json!("EventsNode"));
        node.insert("event".into(), json!(self.event));
        node.insert("name".into(), json!(self.name.clone().unwrap_or_else(|| self.event.clone())));
        node.insert("math".into(), json!(self.math.clone().unwrap_or_else(|| "total".to_string())));
        if let Some(property) = &self.property {
            node.insert("math_property".into(), json!(property));
        }
        if !self.filters.is_empty() {
            node.insert(
                "properties".into(),
                Value::Array(self.filters.iter().map(Filter::to_value).collect()),
            );
        }
        Value::Object(node)
    }

    fn from_value(value: &Value) -> Option<Series> {
        let event = value.get("event").and_then(Value::as_str)?.to_string();
        let name = value.get("name").and_then(Value::as_str).map(str::to_string).filter(|n| n != &event);
        let math = value.get("math").and_then(Value::as_str).map(str::to_string).filter(|m| m != "total");
        let property = value.get("math_property").and_then(Value::as_str).map(str::to_string);
        let filters = value
            .get("properties")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Filter::from_value).collect())
            .unwrap_or_default();
        Some(Series { event, name, math, property, filters })
    }
}

impl Filter {
    fn to_value(&self) -> Value {
        let kind = self.kind.clone().unwrap_or_else(|| "event".to_string());
        if kind == "hogql" {
            return json!({ "key": self.key, "type": "hogql" });
        }
        json!({
            "key": self.key,
            "value": self.value.clone().unwrap_or(Value::Null),
            "operator": self.operator.clone().unwrap_or_else(|| "exact".to_string()),
            "type": kind
        })
    }

    fn from_value(value: &Value) -> Option<Filter> {
        let key = value.get("key").and_then(Value::as_str)?.to_string();
        let kind = value.get("type").and_then(Value::as_str).map(str::to_string).filter(|k| k != "event");
        let operator =
            value.get("operator").and_then(Value::as_str).map(str::to_string).filter(|o| o != "exact");
        let value = value.get("value").cloned().filter(|v| !v.is_null());
        Some(Filter { key, value, operator, kind })
    }
}

/// `14d` → (14, "day"); `2h` → (2, "hour"); `30m` → (30, "minute"); `1w` → (1, "week").
pub fn parse_window(text: &str) -> Result<(u64, &'static str)> {
    let text = text.trim();
    let (digits, unit) = text.split_at(text.trim_end_matches(|c: char| c.is_ascii_alphabetic()).len());
    let number: u64 =
        digits.parse().with_context(|| format!("funnel window {text:?} should look like 14d, 2h or 30m"))?;
    let unit = match unit {
        "m" | "min" | "minute" | "minutes" => "minute",
        "h" | "hour" | "hours" => "hour",
        "d" | "day" | "days" => "day",
        "w" | "week" | "weeks" => "week",
        "M" | "month" | "months" => "month",
        _ => bail!("funnel window {text:?} should end in m, h, d, w or M"),
    };
    Ok((number, unit))
}

fn dedupe(tags: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    tags.into_iter().filter(|tag| seen.insert(tag.clone())).collect()
}

/// A dashboard's slug from its tags, when phog made it.
pub fn slug_of(tags: &[String]) -> Option<String> {
    tags.iter().find_map(|tag| {
        let rest = tag.strip_prefix(&format!("{TAG}:"))?;
        if rest.contains(':') {
            return None;
        }
        Some(rest.to_string())
    })
}

/// An insight's key from its tags, given the dashboard it belongs to.
pub fn key_of(tags: &[String], slug: &str) -> Option<String> {
    let prefix = format!("{TAG}:{slug}:");
    tags.iter().find_map(|tag| tag.strip_prefix(&prefix).map(str::to_string))
}

/// Something usable as a key, from a name PostHog gave an insight.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "insight".to_string()
    } else {
        out
    }
}
