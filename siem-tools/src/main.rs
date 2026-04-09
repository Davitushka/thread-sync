//! SOC CLI utilities (Rust). Prefer this over Python/shell where we ship logic in-repo.
use anyhow::Result;
use clap::{Parser, Subcommand};

mod alert_seed;
mod grafana;

#[derive(Parser)]
#[command(name = "siem-tools", version, about = "SIEM-Lite operator utilities")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Insert synthetic rows into siem.alerts (ClickHouse HTTP).
    AlertSeed(alert_seed::Args),
    /// Add Loki container logs panel to every dashboard in grafana/dashboards (idempotent).
    GrafanaAddLokiPanels(grafana::RepoRootArgs),
    /// Fix ClickHouse formatDateTime patterns in dashboard JSON (%M month → %i minutes).
    GrafanaFixDatetime(grafana::RepoRootArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::AlertSeed(args) => alert_seed::run(args),
        Command::GrafanaAddLokiPanels(args) => grafana::add_loki_panels(args),
        Command::GrafanaFixDatetime(args) => grafana::fix_clickhouse_datetime_in_dashboards(args),
    }
}
