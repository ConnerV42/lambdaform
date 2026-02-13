//! Lambdaform - Terraform-native local Lambda emulator
//!
//! The only local Lambda tool that reads your Terraform.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use walkdir::WalkDir;

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

async fn cmd_start(port: u16, dir: PathBuf, watch: bool) -> anyhow::Result<()> {
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
        println!("\nHint: Make sure your .tf files contain aws_lambda_function resources.");
        println!("      Run `lambdaform validate --dir {}` for details.", dir.display());
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

    if watch {
        println!("\n👀 Hot reload enabled — watching for file changes");
    }

    println!("\n🔥 Starting server at http://localhost:{}\n", port);

    // Start HTTP server (blocks until shutdown)
    if watch {
        server::start_server_with_watch(config, dir, port).await?;
    } else {
        server::start_server(config, dir, port).await?;
    }

    Ok(())
}

fn cmd_invoke(
    function: String,
    event: Option<String>,
    event_file: Option<PathBuf>,
) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        // Parse from current directory
        let dir = PathBuf::from(".");
        let tf_config = parser::parse_terraform_dir(&dir)?;

        if tf_config.functions.is_empty() {
            anyhow::bail!(
                "No Lambda functions found in current directory.\n\
                 Hint: Make sure you have aws_lambda_function resources in your .tf files."
            );
        }

        // Find the function by resource name or function_name
        let lambda = tf_config
            .functions
            .iter()
            .find(|f| f.resource_name == function || f.function_name == function)
            .ok_or_else(|| {
                let available: Vec<_> = tf_config.functions.iter()
                    .map(|f| format!("  • {} ({})", f.resource_name, f.function_name))
                    .collect();
                anyhow::anyhow!(
                    "Function '{}' not found.\n\nAvailable functions:\n{}",
                    function,
                    available.join("\n")
                )
            })?;

        // Parse event JSON
        let event_json = match (event, event_file) {
            (Some(e), _) => {
                // Validate it's valid JSON
                serde_json::from_str::<serde_json::Value>(&e)
                    .map_err(|e| anyhow::anyhow!("Invalid event JSON: {}\nHint: Make sure to quote your JSON properly.", e))?;
                e
            }
            (None, Some(f)) => {
                let content = std::fs::read_to_string(&f)
                    .map_err(|e| anyhow::anyhow!("Cannot read event file '{}': {}", f.display(), e))?;
                serde_json::from_str::<serde_json::Value>(&content)
                    .map_err(|e| anyhow::anyhow!("Invalid JSON in '{}': {}", f.display(), e))?;
                content
            }
            (None, None) => "{}".to_string(),
        };

        println!("⚡ Invoking {} ({:?})", lambda.function_name, lambda.runtime);

        // Build a minimal Lambda event for direct invocation
        let lambda_event = runtime::LambdaEvent {
            http_method: "INVOKE".to_string(),
            path: "/".to_string(),
            path_parameters: None,
            query_string_parameters: None,
            headers: None,
            body: Some(event_json),
            is_base64_encoded: false,
        };

        let executor = runtime::FunctionExecutor::new(lambda.clone(), dir);
        match executor.invoke(lambda_event).await {
            Ok(response) => {
                // Pretty-print the response body
                if let Some(body) = &response.body {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) {
                        println!("{}", serde_json::to_string_pretty(&parsed)?);
                    } else {
                        println!("{}", body);
                    }
                }
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("❌ Invocation failed: {}", e);
            }
        }
    })
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
    println!("🔍 Validating Terraform in: {}\n", dir.display());

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Check directory exists
    if !dir.exists() {
        anyhow::bail!("Directory '{}' does not exist.", dir.display());
    }

    // Check for .tf files
    let tf_files: Vec<_> = WalkDir::new(&dir)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "tf"))
        .collect();

    if tf_files.is_empty() {
        anyhow::bail!(
            "No .tf files found in '{}'.\n\
             Hint: Run this command from your Terraform project root.",
            dir.display()
        );
    }

    println!("   Found {} .tf file(s)", tf_files.len());

    // Try parsing
    let config = match parser::parse_terraform_dir(&dir) {
        Ok(c) => c,
        Err(e) => {
            println!("\n❌ Parse error: {}", e);
            println!("\nHint: Check your HCL syntax. Common issues:");
            println!("  • Missing closing braces");
            println!("  • Unquoted string values");
            println!("  • Invalid block structure");
            return Err(e);
        }
    };

    // Validate functions
    for f in &config.functions {
        // Check handler format
        if !f.handler.contains('.') {
            errors.push(format!(
                "Function '{}': handler '{}' missing dot separator (expected 'file.function')",
                f.function_name, f.handler
            ));
        }

        // Check runtime support
        match &f.runtime {
            config::Runtime::Unknown(r) => {
                warnings.push(format!(
                    "Function '{}': runtime '{}' is not supported yet (supported: nodejs18.x, nodejs20.x, python3.10-3.12)",
                    f.function_name, r
                ));
            }
            config::Runtime::Go1 | config::Runtime::ProvidedAl2 | config::Runtime::ProvidedAl2023 => {
                warnings.push(format!(
                    "Function '{}': runtime {:?} is not yet supported (coming soon)",
                    f.function_name, f.runtime
                ));
            }
            _ => {}
        }

        // Check handler file exists
        let (file, _func) = f.handler.rsplitn(2, '.').collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .windows(2)
            .next()
            .map(|w| (w[0], w[1]))
            .unwrap_or(("", ""));

        if !file.is_empty() {
            let ext = if f.runtime.is_nodejs() { "js" } else if f.runtime.is_python() { "py" } else { "" };
            if !ext.is_empty() {
                let filename = format!("{}.{}", file, ext);
                let found = ["", "src/", "lib/", "lambda/"].iter().any(|prefix| {
                    dir.join(format!("{}{}", prefix, filename)).exists()
                });
                if !found {
                    warnings.push(format!(
                        "Function '{}': handler file '{}' not found in project",
                        f.function_name, filename
                    ));
                }
            }
        }
    }

    // Validate routes point to existing functions
    let fn_names: Vec<&str> = config.functions.iter().map(|f| f.resource_name.as_str()).collect();
    for gw in &config.gateways {
        for route in &gw.routes {
            if !fn_names.contains(&route.function_resource.as_str()) {
                errors.push(format!(
                    "Route {} {} → '{}': function not found (available: {})",
                    route.method_str(), route.path, route.function_resource,
                    fn_names.join(", ")
                ));
            }
        }
    }

    // Print results
    println!("   Found {} function(s), {} gateway(s), {} route(s)",
        config.functions.len(),
        config.gateways.len(),
        config.gateways.iter().map(|g| g.routes.len()).sum::<usize>()
    );

    if !warnings.is_empty() {
        println!("\n⚠️  Warnings:");
        for w in &warnings {
            println!("   • {}", w);
        }
    }

    if !errors.is_empty() {
        println!("\n❌ Errors:");
        for e in &errors {
            println!("   • {}", e);
        }
        anyhow::bail!("Validation failed with {} error(s)", errors.len());
    }

    println!("\n✅ Validation passed!");
    Ok(())
}
