//! Maintain Grafana dashboard JSON under `grafana/dashboards/` (replaces small Python scripts).
use anyhow::{Context, Result};
use clap::Args;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const LOKI_TITLE: &str = "Loki: логи контейнеров SIEM";
const LOKI_MARKDOWN_HINT: &str = "\n\n---\n**Логи Docker:** внизу дашборда панель **«Loki: логи контейнеров SIEM»** (Promtail → Loki). Фильтр по `container` именам контейнеров.";

#[derive(Args, Clone, Debug)]
pub struct RepoRootArgs {
    /// Repository root (directory that contains `grafana/dashboards`). If omitted, walks up from cwd.
    #[arg(long)]
    pub repo_root: Option<PathBuf>,
}

fn resolve_repo_root(args: &RepoRootArgs) -> Result<PathBuf> {
    if let Some(p) = &args.repo_root {
        let p = fs::canonicalize(p).with_context(|| format!("repo root {:?}", p))?;
        return Ok(p);
    }
    let cwd = std::env::current_dir()?;
    find_repo_with_grafana(&cwd).context(
        "could not find grafana/dashboards; run from repo root or pass --repo-root",
    )
}

fn find_repo_with_grafana(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("grafana/dashboards").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn grid_bottom(panel: &Value) -> i64 {
    let g = panel.get("gridPos").and_then(|v| v.as_object());
    let y = g.and_then(|m| m.get("y")).and_then(|v| v.as_i64()).unwrap_or(0);
    let h = g.and_then(|m| m.get("h")).and_then(|v| v.as_i64()).unwrap_or(0);
    y + h
}

fn max_panel_id(panels: &[Value]) -> i64 {
    panels
        .iter()
        .filter_map(|p| p.get("id").and_then(|v| v.as_i64()))
        .max()
        .unwrap_or(0)
}

/// Add a Loki logs panel to each dashboard if missing (idempotent).
pub fn add_loki_panels(args: RepoRootArgs) -> Result<()> {
    let root = resolve_repo_root(&args)?;
    let dash_dir = root.join("grafana/dashboards");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dash_dir)
        .with_context(|| format!("read {}", dash_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    for path in paths {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let mut data: Value = serde_json::from_str(&raw).with_context(|| {
            format!("parse JSON {}", path.display())
        })?;

        let panels = data
            .as_object_mut()
            .with_context(|| format!("{}: root is not an object", path.display()))?
            .entry("panels".to_string())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .with_context(|| format!("{}: panels is not an array", path.display()))?;

        if panels.iter().any(|p| {
            p.get("title").and_then(|t| t.as_str()) == Some(LOKI_TITLE)
        }) {
            eprintln!("skip (already has Loki): {}", path.file_name().unwrap().to_string_lossy());
            continue;
        }

        for p in panels.iter_mut() {
            if p.get("type").and_then(|t| t.as_str()) != Some("text") {
                continue;
            }
            let opts = p
                .as_object_mut()
                .context("panel object")?
                .entry("options".to_string())
                .or_insert_with(|| json!({}));
            let opts_obj = opts.as_object_mut().context("options object")?;
            let content = opts_obj
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            if content.contains("**Логи Docker:**") {
                break;
            }
            opts_obj.insert(
                "content".to_string(),
                Value::String(content.trim_end().to_string() + LOKI_MARKDOWN_HINT),
            );
            break;
        }

        let bottom = panels.iter().map(|p| grid_bottom(p)).max().unwrap_or(0);
        let new_id = max_panel_id(panels) + 1;

        let log_panel = json!({
            "id": new_id,
            "title": LOKI_TITLE,
            "description": "Стрим логов ключевых сервисов из Loki (docker_sd → label container).",
            "type": "logs",
            "gridPos": { "x": 0, "y": bottom, "w": 24, "h": 10 },
            "datasource": { "type": "loki", "uid": "loki-siem" },
            "targets": [{
                "refId": "A",
                "datasource": { "type": "loki", "uid": "loki-siem" },
                "editorMode": "code",
                "expr": "{container=~\"siem-clickhouse|siem-parser|siem-correlator|siem-vector-aggregator|siem-admin|siem-grafana|siem-prometheus|siem-redpanda\"}",
                "queryType": "range",
            }],
            "options": {
                "dedupStrategy": "none",
                "enableLogDetails": true,
                "prettifyLogMessage": false,
                "showCommonLabels": false,
                "showLabels": true,
                "showTime": true,
                "sortOrder": "Descending",
                "wrapLogMessage": true,
            }
        });
        panels.push(log_panel);

        let out = serde_json::to_string_pretty(&data)?;
        fs::write(&path, out + "\n").with_context(|| format!("write {}", path.display()))?;
        eprintln!(
            "updated: {} (panel id={new_id}, y={bottom})",
            path.file_name().unwrap().to_string_lossy()
        );
    }

    Ok(())
}

const DATETIME_REPLS: &[(&str, &str)] = &[
    ("'%Y-%m-%d %H:%M:%S'", "'%Y-%m-%d %H:%i:%S'"),
    ("'%m-%d %H:%M'", "'%m-%d %H:%i'"),
    ("'%H:%M:%S'", "'%H:%i:%S'"),
];

/// Fix ClickHouse formatDateTime in dashboards: `%M` is month name; minutes are `%i`.
pub fn fix_clickhouse_datetime_in_dashboards(args: RepoRootArgs) -> Result<()> {
    let root = resolve_repo_root(&args)?;
    let dash_dir = root.join("grafana/dashboards");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dash_dir)
        .with_context(|| format!("read {}", dash_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    for path in paths {
        let mut text = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let orig = text.clone();
        for (a, b) in DATETIME_REPLS {
            text = text.replace(a, b);
        }
        if text != orig {
            fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
            eprintln!("fixed {}", path.file_name().unwrap().to_string_lossy());
        }
    }

    Ok(())
}
