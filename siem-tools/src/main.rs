//! SOC CLI utilities (Rust). Prefer this over Python/shell where we ship logic in-repo.
use anyhow::Result;
use clap::{Parser, Subcommand};

mod alert_seed;

#[derive(Parser)]
#[command(name = "siem-tools", version, about = "SIEM-Lite operator utilities")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Insert synthetic rows into siem.alerts (same contract as alert-seeder/seed_alerts.py).
    AlertSeed(alert_seed::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::AlertSeed(args) => alert_seed::run(args),
    }
}
