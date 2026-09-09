//! Making PostHog match a dashboard file.
//!
//! Apply is a three-step reconcile: the dashboard itself, then every insight
//! in the file (patch the one the tag finds, create the rest), then the tiles
//! that are managed but no longer in the file. Every step reads before it
//! writes, so the second run finds nothing to do.

use crate::client::Client;
use crate::spec::{self, Dashboard, Insight, Layout};
use anyhow::{bail, Context, Result};
use console::style;
use serde_json::{json, Value};

/// One thing apply would do, so a dry run can say it without doing it.
#[derive(Debug)]
pub enum Change {
    CreateDashboard,
    UpdateDashboard(Vec<&'static str>),
    CreateInsight(String),
    UpdateInsight(String, Vec<&'static str>),
    RemoveInsight(String),
    Layout(String),
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Change::CreateDashboard => write!(f, "{} the dashboard", style("create").green()),
            Change::UpdateDashboard(fields) => {
                write!(f, "{} the dashboard ({})", style("update").yellow(), fields.join(", "))
            }
            Change::CreateInsight(key) => write!(f, "{} insight {key}", style("create").green()),
            Change::UpdateInsight(key, fields) => {
                write!(f, "{} insight {key} ({})", style("update").yellow(), fields.join(", "))
            }
            Change::RemoveInsight(key) => {
                write!(f, "{} insight {key} (no longer in the file)", style("remove").red())
            }
            Change::Layout(key) => write!(f, "{} tile {key}", style("move").cyan()),
        }
    }
}

/// A PostHog dashboard with the tiles that matter to us read out of it.
pub struct Remote {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub tiles: Vec<Tile>,
}

pub struct Tile {
    pub id: u64,
    pub insight_id: u64,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub query: Value,
    pub layout: Option<Layout>,
}

fn tags_of(value: &Value) -> Vec<String> {
    value
        .get("tags")
        .and_then(Value::as_array)
        .map(|t| t.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

fn text_of(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn layout_of(tile: &Value) -> Option<Layout> {
    let sm = tile.pointer("/layouts/sm")?;
    Some(Layout {
        x: sm.get("x")?.as_u64()? as u32,
        y: sm.get("y")?.as_u64()? as u32,
        w: sm.get("w")?.as_u64()? as u32,
        h: sm.get("h")?.as_u64()? as u32,
    })
}

pub fn read(client: &Client, id: u64) -> Result<Remote> {
    let value = client.get(&client.project_path(&format!("dashboards/{id}/")), &[])?;
    Ok(remote_of(&value))
}

fn remote_of(value: &Value) -> Remote {
    let tiles = value
        .get("tiles")
        .and_then(Value::as_array)
        .map(|tiles| {
            tiles
                .iter()
                .filter_map(|tile| {
                    let insight = tile.get("insight")?;
                    if insight.is_null() {
                        return None;
                    }
                    Some(Tile {
                        id: tile.get("id")?.as_u64()?,
                        insight_id: insight.get("id")?.as_u64()?,
                        name: text_of(insight, "name"),
                        description: text_of(insight, "description"),
                        tags: tags_of(insight),
                        query: insight.get("query").cloned().unwrap_or(Value::Null),
                        layout: layout_of(tile),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Remote {
        id: value.get("id").and_then(Value::as_u64).unwrap_or_default(),
        name: text_of(value, "name"),
        description: text_of(value, "description"),
        tags: tags_of(value),
        tiles,
    }
}

/// Every dashboard in the project, one page at a time.
pub fn list(client: &Client) -> Result<Vec<Value>> {
    client.list_all(&client.project_path("dashboards/"), &[("limit", "100".to_string())])
}

/// The dashboard a file describes, if PostHog already has it: by our tag first,
/// then by exact name, which is how a dashboard somebody built by hand gets
/// adopted by a file rather than duplicated beside it.
pub fn find(client: &Client, dashboard: &Dashboard) -> Result<Option<u64>> {
    let tag = dashboard.tag();
    let all = list(client)?;
    let by_tag = all.iter().find(|d| tags_of(d).contains(&tag));
    let by_name = all.iter().find(|d| text_of(d, "name") == dashboard.name);
    Ok(by_tag.or(by_name).and_then(|d| d.get("id").and_then(Value::as_u64)))
}

/// A dashboard by id or by name, for show and export.
pub fn resolve(client: &Client, id_or_name: &str) -> Result<u64> {
    if let Ok(id) = id_or_name.parse::<u64>() {
        return Ok(id);
    }
    let all = list(client)?;
    let wanted = id_or_name.to_lowercase();
    let found = all
        .iter()
        .find(|d| {
            text_of(d, "name").to_lowercase() == wanted
                || spec::slug_of(&tags_of(d)).as_deref() == Some(id_or_name)
        })
        .or_else(|| all.iter().find(|d| text_of(d, "name").to_lowercase().contains(&wanted)));
    match found.and_then(|d| d.get("id").and_then(Value::as_u64)) {
        Some(id) => Ok(id),
        None => bail!("No dashboard called {id_or_name:?}. `phog dashboards` lists them."),
    }
}

/// What apply would do. Reads everything, writes nothing.
pub fn plan(client: &Client, dashboard: &Dashboard) -> Result<(Option<Remote>, Vec<Change>)> {
    let mut changes = Vec::new();
    let remote = match find(client, dashboard)? {
        Some(id) => Some(read(client, id)?),
        None => None,
    };

    match &remote {
        None => changes.push(Change::CreateDashboard),
        Some(remote) => {
            let mut fields = Vec::new();
            if remote.name != dashboard.name {
                fields.push("name");
            }
            if remote.description != dashboard.description.clone().unwrap_or_default() {
                fields.push("description");
            }
            if !same_set(&remote.tags, &dashboard.all_tags()) {
                fields.push("tags");
            }
            if !fields.is_empty() {
                changes.push(Change::UpdateDashboard(fields));
            }
        }
    }

    for insight in &dashboard.insights {
        let query = insight.to_query(dashboard)?;
        let existing =
            remote.as_ref().and_then(|r| r.tiles.iter().find(|t| t.tags.contains(&insight.tag(dashboard))));
        match existing {
            None => {
                changes.push(Change::CreateInsight(insight.key.clone()));
                if insight.layout.is_some() {
                    changes.push(Change::Layout(insight.key.clone()));
                }
            }
            Some(tile) => {
                let mut fields = Vec::new();
                if tile.name != insight.name {
                    fields.push("name");
                }
                if tile.description != insight.description.clone().unwrap_or_default() {
                    fields.push("description");
                }
                if !same_set(&tile.tags, &insight.all_tags(dashboard)) {
                    fields.push("tags");
                }
                if !same_query(&tile.query, &query) {
                    fields.push("query");
                }
                if !fields.is_empty() {
                    changes.push(Change::UpdateInsight(insight.key.clone(), fields));
                }
                if let Some(layout) = insight.layout {
                    if tile.layout != Some(layout) {
                        changes.push(Change::Layout(insight.key.clone()));
                    }
                }
            }
        }
    }

    if let Some(remote) = &remote {
        for tile in &remote.tiles {
            if let Some(key) = spec::key_of(&tile.tags, &dashboard.slug) {
                if !dashboard.insights.iter().any(|i| i.key == key) {
                    changes.push(Change::RemoveInsight(key));
                }
            }
        }
    }

    Ok((remote, changes))
}

/// Make PostHog match the file. Returns the dashboard id.
pub fn apply(client: &Client, dashboard: &Dashboard, prune: bool) -> Result<u64> {
    let (remote, _) = plan(client, dashboard)?;
    let dashboards_path = client.project_path("dashboards/");
    let insights_path = client.project_path("insights/");

    let id = match &remote {
        Some(remote) => {
            let body = json!({ "name": dashboard.name, "description": dashboard.description.clone().unwrap_or_default(), "tags": dashboard.all_tags() });
            client.patch(&format!("{dashboards_path}{}/", remote.id), &body)?;
            remote.id
        }
        None => {
            let body = json!({ "name": dashboard.name, "description": dashboard.description.clone().unwrap_or_default(), "tags": dashboard.all_tags() });
            let created = client.post(&dashboards_path, &body)?;
            created
                .get("id")
                .and_then(Value::as_u64)
                .context("PostHog created the dashboard but returned no id")?
        }
    };
    println!("  {} dashboard {} ({})", style("✓").green(), dashboard.name, client.dashboard_url(id));

    let current = read(client, id)?;
    for insight in &dashboard.insights {
        let query = insight.to_query(dashboard)?;
        let body = json!({
            "name": insight.name,
            "description": insight.description.clone().unwrap_or_default(),
            "tags": insight.all_tags(dashboard),
            "query": query,
            "dashboards": [id]
        });
        match current.tiles.iter().find(|t| t.tags.contains(&insight.tag(dashboard))) {
            Some(tile) => {
                let changed = tile.name != insight.name
                    || tile.description != insight.description.clone().unwrap_or_default()
                    || !same_set(&tile.tags, &insight.all_tags(dashboard))
                    || !same_query(&tile.query, &query);
                if changed {
                    client.patch(&format!("{insights_path}{}/", tile.insight_id), &body)?;
                    println!("  {} updated {}", style("✓").yellow(), insight.key);
                } else {
                    println!("  {} {} unchanged", style("·").dim(), insight.key);
                }
            }
            None => {
                client.post(&insights_path, &body)?;
                println!("  {} created {}", style("✓").green(), insight.key);
            }
        }
    }

    if prune {
        for tile in &current.tiles {
            if let Some(key) = spec::key_of(&tile.tags, &dashboard.slug) {
                if !dashboard.insights.iter().any(|i| i.key == key) {
                    client.patch(
                        &format!("{insights_path}{}/", tile.insight_id),
                        &json!({ "deleted": true }),
                    )?;
                    println!("  {} removed {}", style("✓").red(), key);
                }
            }
        }
    }

    place(client, dashboard, id)?;
    Ok(id)
}

/// Put every tile with a layout where the file says. PostHog keeps two grids,
/// one per breakpoint; the small one is what a laptop shows.
fn place(client: &Client, dashboard: &Dashboard, id: u64) -> Result<()> {
    let current = read(client, id)?;
    let mut tiles = Vec::new();
    for insight in &dashboard.insights {
        let layout = match insight.layout {
            Some(layout) => layout,
            None => continue,
        };
        let tile = match current.tiles.iter().find(|t| t.tags.contains(&insight.tag(dashboard))) {
            Some(tile) => tile,
            None => continue,
        };
        if tile.layout == Some(layout) {
            continue;
        }
        let grid = json!({ "i": tile.id.to_string(), "x": layout.x, "y": layout.y, "w": layout.w, "h": layout.h, "minW": 3, "minH": 3 });
        tiles.push(json!({ "id": tile.id, "layouts": { "sm": grid, "xs": { "i": tile.id.to_string(), "x": 0, "y": layout.y, "w": 1, "h": layout.h } } }));
    }
    if tiles.is_empty() {
        return Ok(());
    }
    client.patch(&client.project_path(&format!("dashboards/{id}/")), &json!({ "tiles": tiles }))?;
    println!(
        "  {} placed {} tile{}",
        style("✓").cyan(),
        tiles.len(),
        if tiles.len() == 1 { "" } else { "s" }
    );
    Ok(())
}

/// A dashboard PostHog holds, as a file. Managed insights keep their keys;
/// hand-made ones get a key from their name, so the export can be applied back.
pub fn export(client: &Client, id: u64) -> Result<Dashboard> {
    let remote = read(client, id)?;
    let slug = spec::slug_of(&remote.tags).unwrap_or_else(|| spec::slugify(&remote.name));
    let tags: Vec<String> = remote
        .tags
        .iter()
        .filter(|t| !t.starts_with(&format!("{}:", spec::TAG)) && *t != spec::TAG)
        .cloned()
        .collect();
    let mut used = std::collections::HashSet::new();
    let insights = remote
        .tiles
        .iter()
        .map(|tile| {
            let mut key = spec::key_of(&tile.tags, &slug).unwrap_or_else(|| spec::slugify(&tile.name));
            let base = key.clone();
            let mut n = 2;
            while !used.insert(key.clone()) {
                key = format!("{base}-{n}");
                n += 1;
            }
            let description = Some(tile.description.clone()).filter(|d| !d.is_empty());
            let mut insight = Insight::from_query(&key, &tile.name, description, &tile.query);
            insight.layout = tile.layout;
            insight
        })
        .collect();
    Ok(Dashboard {
        name: remote.name,
        slug,
        description: Some(remote.description).filter(|d| !d.is_empty()),
        tags,
        date_from: None,
        insights,
    })
}

pub fn delete(client: &Client, id: u64) -> Result<()> {
    client.patch(&client.project_path(&format!("dashboards/{id}/")), &json!({ "deleted": true }))?;
    Ok(())
}

fn same_set(a: &[String], b: &[String]) -> bool {
    let mut a: Vec<&String> = a.iter().collect();
    let mut b: Vec<&String> = b.iter().collect();
    a.sort();
    b.sort();
    a == b
}

/// PostHog fills a stored query with defaults we never sent, so compare only
/// what the file said: every key we set must read back the same.
fn same_query(remote: &Value, wanted: &Value) -> bool {
    subset(wanted, remote)
}

fn subset(wanted: &Value, remote: &Value) -> bool {
    match (wanted, remote) {
        (Value::Object(w), Value::Object(r)) => {
            w.iter().all(|(key, value)| r.get(key).map(|other| subset(value, other)).unwrap_or(false))
        }
        (Value::Array(w), Value::Array(r)) => {
            w.len() == r.len() && w.iter().zip(r).all(|(a, b)| subset(a, b))
        }
        (Value::Number(w), Value::Number(r)) => w.as_f64() == r.as_f64(),
        _ => wanted == remote,
    }
}
