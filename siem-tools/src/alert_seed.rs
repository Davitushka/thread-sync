use std::collections::HashMap;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::Parser;
use rand::prelude::*;
use reqwest::blocking::Client;

#[derive(Parser)]
pub struct Args {
    /// ClickHouse HTTP URL (e.g. http://clickhouse:8123)
    #[arg(long, env = "CLICKHOUSE_URL", default_value = "http://clickhouse:8123")]
    clickhouse_url: String,

    #[arg(long, env = "CLICKHOUSE_USER", default_value = "siem")]
    clickhouse_user: String,

    #[arg(long, env = "CLICKHOUSE_PASSWORD", default_value = "ClickHousePass123!")]
    clickhouse_password: String,

    #[arg(long, env = "ALERT_SEEDER_TOTAL", default_value_t = 50)]
    total: usize,
}

struct RuleMeta {
    title: &'static str,
    mitre: &'static [&'static str],
    descriptions: &'static [&'static str],
}

fn rules() -> HashMap<&'static str, RuleMeta> {
    HashMap::from([
        (
            "brute_force_api",
            RuleMeta {
                title: "API / SignalR Brute-Force Authentication Attempts",
                mitre: &["T1110", "T1110.001"],
                descriptions: &[
                    "Brute-force detected against API authentication endpoints",
                    "Multiple failed authentication attempts from single IP",
                ],
            },
        ),
        (
            "sql_injection_attempt",
            RuleMeta {
                title: "SQL/NoSQL Injection Attempt Detected in Application Logs",
                mitre: &["T1190", "T1059.007"],
                descriptions: &[
                    "Union-based SQL injection payload detected in request path",
                    "Database error pattern indicates probable SQL injection attempt",
                ],
            },
        ),
        (
            "privilege_escalation_attempt",
            RuleMeta {
                title: "Privilege Escalation or Unauthorized Admin Access Attempt",
                mitre: &["T1068", "T1078.003", "T1548"],
                descriptions: &[
                    "Forbidden access to privileged endpoint detected",
                    "Repeated unauthorized access to admin resources",
                ],
            },
        ),
        (
            "rate_limit_evasion",
            RuleMeta {
                title: "Rate Limit Evasion - Anomalous Request Volume from Single IP",
                mitre: &["T1595", "T1595.002"],
                descriptions: &[
                    "Request volume exceeded rate-limiting thresholds",
                    "Burst traffic pattern indicates automated probing activity",
                ],
            },
        ),
    ])
}

const ATTACKER_IPS: &[&str] = &[
    "203.0.113.5",
    "203.0.113.12",
    "198.51.100.20",
    "203.0.113.88",
];

fn random_ts_within_last_days<R: Rng + ?Sized>(rng: &mut R, days: i64) -> chrono::DateTime<Utc> {
    let now = Utc::now();
    let delta = Duration::days(rng.random_range(0..=days))
        + Duration::hours(rng.random_range(0..24))
        + Duration::minutes(rng.random_range(0..60))
        + Duration::seconds(rng.random_range(0..60))
        + Duration::milliseconds(rng.random_range(0..1000));
    now - delta
}

fn ch_ts(dt: chrono::DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

fn status_plan<R: Rng + ?Sized>(rng: &mut R, total: usize) -> Vec<(&'static str, &'static str)> {
    let low_med = ["low", "medium"];
    let mut items: Vec<(&str, &str)> = Vec::new();
    items.extend(std::iter::repeat(("new", "critical")).take(10));
    items.extend(std::iter::repeat(("new", "high")).take(10));
    items.extend(std::iter::repeat(("acknowledged", "medium")).take(15));
    for _ in 0..10 {
        let sev = *low_med.choose(rng).unwrap();
        items.push(("resolved", sev));
    }
    for _ in 0..5 {
        let sev = *low_med.choose(rng).unwrap();
        items.push(("false_positive", sev));
    }
    items.truncate(total);
    items
}

#[derive(Debug)]
struct AlertRow {
    alert_id: String,
    fingerprint: String,
    triggered_at: String,
    rule_id: String,
    rule_title: String,
    severity: String,
    description: String,
    source_ip: String,
    user_id: Option<String>,
    event_ids: Vec<String>,
    mitre_tags: Vec<String>,
    status: String,
    acknowledged_by: Option<String>,
    acknowledged_at: Option<String>,
    notes: String,
}

fn build_row<R: Rng + ?Sized>(
    rng: &mut R,
    rules: &HashMap<&str, RuleMeta>,
    status: &str,
    severity: &str,
) -> AlertRow {
    let rule_ids: Vec<&str> = rules.keys().copied().collect();
    let rule_id = *rule_ids.choose(rng).unwrap();
    let rule = &rules[rule_id];
    let triggered_at = random_ts_within_last_days(rng, 7);
    let mut acknowledged_at: Option<chrono::DateTime<Utc>> = None;
    let mut acknowledged_by: Option<String> = None;

    if status == "acknowledged" || status == "resolved" {
        acknowledged_at = Some(
            triggered_at
                + Duration::try_minutes(rng.random_range(5..=300)).unwrap_or_else(|| Duration::zero()),
        );
        acknowledged_by = Some(
            ["soc_analyst_1", "soc_analyst_2", "incident_bot"]
                .choose(rng)
                .unwrap()
                .to_string(),
        );
    }

    let user_pool: [Option<&str>; 4] = [Some("admin"), Some("svc-api"), Some("user_023"), None];
    let user_id = user_pool.choose(rng).unwrap().map(|s| (*s).to_string());

    AlertRow {
        alert_id: uuid::Uuid::new_v4().to_string(),
        fingerprint: uuid::Uuid::new_v4().to_string(),
        triggered_at: ch_ts(triggered_at),
        rule_id: rule_id.to_string(),
        rule_title: rule.title.to_string(),
        severity: severity.to_string(),
        description: (*rule.descriptions.choose(rng).unwrap()).to_string(),
        source_ip: (*ATTACKER_IPS.choose(rng).unwrap()).to_string(),
        user_id,
        event_ids: (0..rng.random_range(1..=3))
            .map(|_| uuid::Uuid::new_v4().to_string())
            .collect(),
        mitre_tags: rule.mitre.iter().map(|s| (*s).to_string()).collect(),
        status: status.to_string(),
        acknowledged_by,
        acknowledged_at: acknowledged_at.map(ch_ts),
        notes: [
            "Seeded alert for Grafana validation",
            "Synthetic SOC alert for dashboard population",
            "Generated by siem-tools alert-seed",
        ]
        .choose(rng)
        .unwrap()
        .to_string(),
    }
}

fn sql_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn row_to_sql_parts(row: &AlertRow) -> String {
    let null = "NULL".to_string();
    let ev = row
        .event_ids
        .iter()
        .map(|x| format!("'{}'", sql_escape(x)))
        .collect::<Vec<_>>()
        .join(", ");
    let mt = row
        .mitre_tags
        .iter()
        .map(|x| format!("'{}'", sql_escape(x)))
        .collect::<Vec<_>>()
        .join(", ");
    let user_sql = row
        .user_id
        .as_ref()
        .map(|u| format!("'{}'", sql_escape(u)))
        .unwrap_or_else(|| null.clone());
    let ack_at = row
        .acknowledged_at
        .as_ref()
        .map(|a| format!("'{}'", sql_escape(a)))
        .unwrap_or_else(|| null.clone());
    let ack_by = row
        .acknowledged_by
        .as_ref()
        .map(|a| format!("'{}'", sql_escape(a)))
        .unwrap_or_else(|| null.clone());

    format!(
        "('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', {}, [{}], [{}], '{}', {}, {}, '{}')",
        sql_escape(&row.alert_id),
        sql_escape(&row.fingerprint),
        sql_escape(&row.triggered_at),
        sql_escape(&row.rule_id),
        sql_escape(&row.rule_title),
        sql_escape(&row.severity),
        sql_escape(&row.description),
        sql_escape(&row.source_ip),
        user_sql,
        ev,
        mt,
        sql_escape(&row.status),
        ack_by,
        ack_at,
        sql_escape(&row.notes),
    )
}

pub fn run(args: Args) -> Result<()> {
    let rules = rules();
    let mut rng = rand::rng();
    let plan = status_plan(&mut rng, args.total);
    let rows: Vec<AlertRow> = plan
        .iter()
        .map(|(st, sev)| build_row(&mut rng, &rules, st, sev))
        .collect();

    let values = rows.iter().map(row_to_sql_parts).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "INSERT INTO siem.alerts (alert_id, fingerprint, triggered_at, rule_id, rule_title, severity, description, source_ip, user_id, event_ids, mitre_tags, status, acknowledged_by, acknowledged_at, notes) VALUES {values}"
    );

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("reqwest client")?;

    let resp = client
        .post(&args.clickhouse_url)
        .basic_auth(&args.clickhouse_user, Some(&args.clickhouse_password))
        .body(sql)
        .send()
        .context("clickhouse POST")?;

    resp.error_for_status().context("clickhouse response")?;

    let new_c = rows.iter().filter(|r| r.status == "new").count();
    let ack_c = rows.iter().filter(|r| r.status == "acknowledged").count();
    let res_c = rows.iter().filter(|r| r.status == "resolved").count();
    let fp_c = rows.iter().filter(|r| r.status == "false_positive").count();

    println!(
        "{}",
        serde_json::json!({
            "seeded": rows.len(),
            "statuses": {
                "new": new_c,
                "acknowledged": ack_c,
                "resolved": res_c,
                "false_positive": fp_c,
            },
            "clickhouse_url": args.clickhouse_url,
        })
    );

    Ok(())
}
