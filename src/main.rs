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
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_start(port, dir, watch))
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

async fn cmd_start(port: u16, dir: PathBuf, _watch: bool) -> anyhow::Result<()> {
    println!(
        r#"
┌─────────────────────────────────────────┐
│           🚀 Lambdaform v0.1.0          │
│     Terraform-native Lambda emulator    │
└─────────────────────────────────────────┘
"#
    );

    println!("📂 Loading Terraform from: {}", dir.display());

    // Parse Terraform files
    let config = parser::parse_terraform_dir(&dir)?;

    if config.functions.is_empty() {
        println!("⚠️  No Lambda functions found in {}", dir.display());
        return Ok(());
    }

    // Log discovered functions
    println!("\n📦 Lambda Functions:");
    for f in &config.functions {
        println!("   • {} ({:?}) → {}", f.function_name, f.runtime, f.handler);
    }

    // Log discovered routes
    if !config.gateways.is_empty() {
        println!("\n🌐 Routes:");
        for gw in &config.gateways {
            for route in &gw.routes {
                println!("   {:?} {} → {}", route.method, route.path, route.function_resource);
            }
        }
    } else {
        println!("\n⚠️  No API Gateway routes found — functions available via `lambdaform invoke` only");
    }

    println!("\n🔥 Starting server at http://localhost:{}\n", port);

    // Start HTTP server (blocks until shutdown)
    server::start_server(config, dir, port).await?;

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

fn cmd_config(json_output: bool, dir: PathBuf) -> anyhow::Result<()> {
    let config = parser::parse_terraform_dir(&dir)?;
    
    if json_output {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        println!("📂 Parsed from: {}\n", dir.display());
        
        if config.functions.is_empty() {
            println!("⚠️  No Lambda functions found");
        } else {
            println!("📦 Lambda Functions ({}):", config.functions.len());
            for f in &config.functions {
                println!("   • {} ({:?})", f.function_name, f.runtime);
                println!("     Handler: {}", f.handler);
                println!("     Timeout: {}s, Memory: {}MB", f.timeout, f.memory_size);
                if !f.environment.is_empty() {
                    println!("     Env vars: {:?}", f.environment.keys().collect::<Vec<_>>());
                }
            }
        }
        
        println!();
        
        if config.gateways.is_empty() {
            println!("⚠️  No API Gateways found");
        } else {
            println!("🌐 API Gateways ({}):", config.gateways.len());
            for gw in &config.gateways {
                println!("   • {} ({:?})", gw.name, gw.api_type);
                if !gw.routes.is_empty() {
                    for route in &gw.routes {
                        println!("     {:?} {} → {}", route.method, route.path, route.function_resource);
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn cmd_validate(dir: PathBuf) -> anyhow::Result<()> {
    println!("Validating Terraform in: {}", dir.display());
    println!("\n⚠️  Not yet implemented - this is the MVP skeleton");

    Ok(())
}
