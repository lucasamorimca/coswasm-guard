mod commands;
mod output;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "cosmwasm-guard")]
#[command(about = "Static analysis for CosmWasm smart contracts")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze CosmWasm contract(s) for vulnerabilities
    Analyze {
        /// Path to .rs file or directory containing CosmWasm contract
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "text")]
        format: OutputFormat,

        /// Minimum severity to report
        #[arg(short, long, default_value = "low")]
        severity: SeverityFilter,

        /// Run only these detectors (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        detectors: Option<Vec<String>>,

        /// Exclude these detectors (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,

        /// Suppress banner and summary
        #[arg(short, long)]
        quiet: bool,

        /// Disable colored output
        #[arg(long)]
        no_color: bool,
    },
    /// List all available detectors
    List,
}

#[derive(ValueEnum, Clone)]
enum OutputFormat {
    Text,
    Json,
    Sarif,
}

#[derive(ValueEnum, Clone)]
enum SeverityFilter {
    High,
    Medium,
    Low,
    Info,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            path,
            format,
            severity,
            detectors,
            exclude,
            quiet,
            no_color,
        } => commands::analyze::run(&path, format, severity, detectors, exclude, quiet, no_color),
        Commands::List => commands::list::run(),
    }
}
