//! Lambdaform - Terraform-native local Lambda emulator
//!
//! The only local Lambda tool that reads your Terraform.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod config;
mod parser;
mod router;
mod runtime;
mod server;
mod watcher;

/// Terraform-native local Lambda emulator
#[derive(Parser)]
#[command(name = "lambdaform")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the local development server
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Enable hot reload
        #[arg(long, default_value = "true")]
        watch: bool,
    },

    /// Invoke a Lambda function directly
    Invoke {
        /// Function name (as defined in Terraform)
        function: String,

        /// Event JSON (inline)
        #[arg(short, long)]
        event: Option<String>,

        /// Event JSON file
        #[arg(short = 'f', long)]
        event_file: Option<PathBuf>,
    },

    /// Show parsed configuration
    Config {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
    },

    /// Validate Terraform files
    Validate {
        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { port, dir, watch } => {
            cmd_start(port, dir, watch)
        }
        Commands::Invoke {
            function,
            event,
            event_file,
        } => {
            cmd_invoke(function, event, event_file)
        }
        Commands::Config { json, dir } => {
            cmd_config(json, dir)
        }
        Commands::Validate { dir } => {
            cmd_validate(dir)
        }
    }
}

fn cmd_start(port: u16, dir: PathBuf, watch: bool) -> anyhow::Result<()> {
    println!(
        r#"
┌─────────────────────────────────────────┐
│           🚀 Lambdaform v0.1.0          │
│     Terraform-native Lambda emulator    │
└─────────────────────────────────────────┘
"#
    );

    // TODO: Parse Terraform files
    // TODO: Build route table
    // TODO: Start HTTP server
    // TODO: Start file watcher

    println!("📂 Loading Terraform from: {}", dir.display());
    println!("🔥 Server would run at http://localhost:{}", port);
    
    if watch {
        println!("👀 Watching for changes...");
    }

    // Placeholder - actual implementation coming
    println!("\n⚠️  Not yet implemented - this is the MVP skeleton");

    Ok(())
}

fn cmd_invoke(
    function: String,
    event: Option<String>,
    event_file: Option<PathBuf>,
) -> anyhow::Result<()> {
    println!("Invoking function: {}", function);

    let event_json = match (event, event_file) {
        (Some(e), _) => e,
        (None, Some(f)) => std::fs::read_to_string(f)?,
        (None, None) => "{}".to_string(),
    };

    println!("Event: {}", event_json);
    println!("\n⚠️  Not yet implemented - this is the MVP skeleton");

    Ok(())
}

fn cmd_config(json: bool, dir: PathBuf) -> anyhow::Result<()> {
    println!("Parsing config from: {}", dir.display());
    println!("JSON output: {}", json);
    println!("\n⚠️  Not yet implemented - this is the MVP skeleton");

    Ok(())
}

fn cmd_validate(dir: PathBuf) -> anyhow::Result<()> {
    println!("Validating Terraform in: {}", dir.display());
    println!("\n⚠️  Not yet implemented - this is the MVP skeleton");

    Ok(())
}
