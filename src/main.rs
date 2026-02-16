//! Lambdaform - Terraform-native local Lambda emulator
//!
//! The only local Lambda tool that reads your Terraform.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use walkdir::WalkDir;

use lambdaform::config;
use lambdaform::parser;
use lambdaform::project_config;
use lambdaform::runtime;
use lambdaform::server;
use lambdaform::stepfunctions;
use lambdaform::trigger;
use lambdaform::websocket;

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

        /// Verbose logging (show headers, bodies, debug info)
        #[arg(short, long)]
        verbose: bool,

        /// Enable Node.js debugger (--inspect-brk)
        #[arg(long)]
        debug: bool,

        /// Node.js debugger port (default: 9229)
        #[arg(long, default_value = "9229")]
        debug_port: u16,

        /// Enable Python debugger (debugpy)
        #[arg(long)]
        debug_python: bool,

        /// Python debugger port (default: 5678)
        #[arg(long, default_value = "5678")]
        debug_python_port: u16,

        /// Additional .tfvars files to load (like terraform -var-file)
        #[arg(long = "var-file", value_name = "FILE")]
        var_files: Vec<PathBuf>,
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

        /// Additional .tfvars files to load (like terraform -var-file)
        #[arg(long = "var-file", value_name = "FILE")]
        var_files: Vec<PathBuf>,
    },

    /// Validate Terraform files
    Validate {
        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Additional .tfvars files to load (like terraform -var-file)
        #[arg(long = "var-file", value_name = "FILE")]
        var_files: Vec<PathBuf>,
    },

    /// Send a test message through SQS/SNS trigger to invoke a Lambda
    Trigger {
        /// Trigger type: sqs or sns
        #[arg(short = 't', long, value_name = "TYPE")]
        source_type: String,

        /// Source resource name or queue/topic name
        #[arg(short, long)]
        source: String,

        /// Message body (string or JSON)
        #[arg(short, long)]
        message: String,

        /// Number of messages in batch (repeats the same message)
        #[arg(short, long, default_value = "1")]
        batch: u32,

        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Override target function (skip event source mapping lookup)
        #[arg(short, long)]
        function: Option<String>,
    },

    /// Visualize Step Functions state machines (read-only)
    #[command(name = "stepfunctions", alias = "sfn")]
    StepFunctions {
        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Show only a specific state machine by name
        #[arg(short, long)]
        name: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging — verbose flag sets DEBUG level
    let verbose = matches!(&cli.command, Commands::Start { verbose: true, .. });
    let default_level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(default_level.into()),
        )
        .init();

    match cli.command {
        Commands::Start {
            port,
            dir,
            watch,
            verbose: _,
            debug,
            debug_port,
            debug_python,
            debug_python_port,
            var_files,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_start(
                port,
                dir,
                watch,
                debug,
                debug_port,
                debug_python,
                debug_python_port,
                var_files,
            ))
        }
        Commands::Invoke {
            function,
            event,
            event_file,
        } => cmd_invoke(function, event, event_file),
        Commands::Config {
            json,
            dir,
            var_files,
        } => cmd_config(json, dir, var_files),
        Commands::Validate { dir, var_files } => cmd_validate(dir, var_files),
        Commands::Trigger {
            source_type,
            source,
            message,
            batch,
            dir,
            function,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_trigger(
                source_type,
                source,
                message,
                batch,
                dir,
                function,
            ))
        }
        Commands::StepFunctions { dir, name, json } => cmd_stepfunctions(dir, name, json),
    }
}

#[allow(clippy::too_many_arguments)]
async fn cmd_start(
    port: u16,
    dir: PathBuf,
    watch: bool,
    debug: bool,
    debug_port: u16,
    debug_python: bool,
    debug_python_port: u16,
    var_files: Vec<PathBuf>,
) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "\n┌─────────────────────────────────────────┐\n│     \
         🚀 Lambdaform v{:<25}│\n│     \
         Terraform-native Lambda emulator    │\n\
         └─────────────────────────────────────────┘\n",
        version
    );

    println!("📂 Loading Terraform from: {}", dir.display());

    // Parse Terraform files (with any --var-file paths)
    let mut config = parser::parse_terraform_dir_with_var_files(&dir, &var_files)?;

    // Load and apply project config (lambdaform.yaml)
    let project_config = project_config::ProjectConfig::load(&dir)?;
    if let Some(ref pc) = project_config {
        println!("📄 Loaded lambdaform.yaml");
        pc.apply(&mut config);
    }
    let port = if port != 3000 {
        port // CLI explicitly set
    } else {
        project_config
            .as_ref()
            .and_then(|pc| pc.port)
            .unwrap_or(port)
    };
    let watch = if !watch {
        false // CLI explicitly disabled
    } else {
        project_config
            .as_ref()
            .and_then(|pc| pc.watch)
            .unwrap_or(watch)
    };

    if config.functions.is_empty() {
        println!("⚠️  No Lambda functions found in {}", dir.display());
        println!("\nHint: Make sure your .tf files contain aws_lambda_function resources.");
        println!(
            "      Run `lambdaform validate --dir {}` for details.",
            dir.display()
        );
        return Ok(());
    }

    // Log discovered functions
    println!("\n📦 Lambda Functions:");
    for f in &config.functions {
        println!("   • {} ({:?}) → {}", f.function_name, f.runtime, f.handler);
    }

    // Compute gateway bindings (each gateway gets its own port when multiple exist)
    let multi_gateway = config.gateways.len() > 1;
    let gateway_bindings: Vec<server::GatewayBinding> = if multi_gateway {
        config
            .gateways
            .iter()
            .enumerate()
            .map(|(i, gw)| {
                let gw_port = project_config
                    .as_ref()
                    .and_then(|pc| pc.gateways.get(&gw.resource_name))
                    .and_then(|ovr| ovr.port)
                    .unwrap_or(port + i as u16);
                server::GatewayBinding {
                    gateway_name: gw.name.clone(),
                    gateway_resource: gw.resource_name.clone(),
                    port: gw_port,
                }
            })
            .collect()
    } else {
        vec![]
    };

    // Log discovered routes
    if !config.gateways.is_empty() {
        if multi_gateway {
            println!("\n🌐 API Gateways ({}):", config.gateways.len());
            for binding in &gateway_bindings {
                let gw = config
                    .gateways
                    .iter()
                    .find(|g| g.resource_name == binding.gateway_resource)
                    .expect("Gateway binding references non-existent gateway");
                println!(
                    "   📡 {} ({:?}) → http://localhost:{}",
                    gw.name, gw.api_type, binding.port
                );
                for route in &gw.routes {
                    println!(
                        "      {:?} {} → {}",
                        route.method, route.path, route.function_resource
                    );
                }
            }
        } else {
            println!("\n🌐 Routes:");
            for gw in &config.gateways {
                for route in &gw.routes {
                    println!(
                        "   {:?} {} → {}",
                        route.method, route.path, route.function_resource
                    );
                }
            }
        }
    } else {
        println!(
            "\n⚠️  No API Gateway routes found — functions available via `lambdaform invoke` only"
        );
    }

    if watch {
        println!("\n👀 Hot reload enabled — watching for file changes");
    }

    if multi_gateway {
        println!("\n🔥 Starting {} servers...\n", gateway_bindings.len());
    } else {
        println!("\n🔥 Starting server at http://localhost:{}\n", port);
    }

    // CORS config
    let cors_config = project_config.as_ref().and_then(|pc| pc.cors.clone());
    if cors_config.is_some() {
        println!("🔓 CORS enabled via lambdaform.yaml");
    } else {
        println!("🔓 CORS enabled (permissive defaults — allow all origins)");
    }

    // DynamoDB Local integration hints
    if !config.dynamodb_tables.is_empty() {
        println!(
            "\n🗄️  DynamoDB tables detected ({}):",
            config.dynamodb_tables.len()
        );
        for table in &config.dynamodb_tables {
            let keys = match (&table.hash_key, &table.range_key) {
                (Some(hk), Some(rk)) => format!("{} + {}", hk, rk),
                (Some(hk), None) => hk.clone(),
                _ => "?".to_string(),
            };
            println!("   • {} [{}]", table.name, keys);
        }
        println!("   💡 Tip: docker run -p 8000:8000 amazon/dynamodb-local");
        println!("   💡 Set AWS_ENDPOINT_URL=http://localhost:8000 in lambdaform.yaml");
    }

    // Build debug options from CLI flags and/or project config
    let debug_from_config = project_config.as_ref().and_then(|pc| pc.debug.clone());

    let has_any_debug = debug
        || debug_python
        || debug_from_config
            .as_ref()
            .is_some_and(|dc| dc.nodejs || dc.python);

    let debug_options = if has_any_debug {
        let dc = debug_from_config.unwrap_or_default();
        Some(runtime::DebugOptions {
            nodejs: debug || dc.nodejs,
            python: debug_python || dc.python,
            port: if debug { debug_port } else { dc.port },
            python_port: if debug_python {
                debug_python_port
            } else {
                dc.python_port
            },
            break_on_start: if debug || debug_python {
                true
            } else {
                dc.break_on_start
            },
        })
    } else {
        None
    };

    if let Some(ref opts) = debug_options {
        if opts.nodejs {
            println!("\n🔍 Node.js debugger enabled on port {}", opts.port);
            if opts.break_on_start {
                println!("   Mode: --inspect-brk (pauses on first line)");
            } else {
                println!("   Mode: --inspect (runs immediately)");
            }
            println!("   Attach: chrome://inspect or VS Code \"Attach to Node.js\"");
        }
        if opts.python {
            println!(
                "\n🐍 Python debugger (debugpy) enabled on port {}",
                opts.python_port
            );
            if opts.break_on_start {
                println!("   Mode: wait_for_client (pauses until debugger attaches)");
            } else {
                println!("   Mode: listen only (runs immediately)");
            }
            println!(
                "   Attach: VS Code \"Python: Remote Attach\" → localhost:{}",
                opts.python_port
            );
        }
    }

    // Separate WebSocket gateways from HTTP gateways
    let ws_gateways: Vec<_> = config
        .gateways
        .iter()
        .filter(|g| g.api_type == config::ApiType::WebSocket)
        .cloned()
        .collect();

    let mut ws_handles = Vec::new();

    for (i, ws_gw) in ws_gateways.iter().enumerate() {
        let ws_port = project_config
            .as_ref()
            .and_then(|pc| pc.gateways.get(&ws_gw.resource_name))
            .and_then(|ovr| ovr.port)
            .unwrap_or(port + 100 + i as u16); // Default: base port + 100

        let route_selection = ws_gw
            .route_selection_expression
            .clone()
            .unwrap_or_else(|| "$request.body.action".to_string());

        println!(
            "\n🔌 WebSocket Gateway: {} → ws://localhost:{}",
            ws_gw.name, ws_port
        );
        println!("   Route selection: {}", route_selection);
        for route in &ws_gw.routes {
            println!("   {} → {}", route.path, route.function_resource);
        }
        println!("   @connections API → http://localhost:{}", ws_port + 1000);

        let ws_config = config.clone();
        let ws_dir = dir.clone();
        let ws_resource = ws_gw.resource_name.clone();
        let ws_debug = debug_options.clone();

        let handle = tokio::spawn(async move {
            websocket::start_websocket_server(
                ws_config,
                &ws_resource,
                ws_dir,
                ws_port,
                route_selection,
                ws_debug,
            )
            .await
        });
        ws_handles.push(handle);
    }

    // Filter HTTP-only gateway bindings
    let http_bindings: Vec<_> = gateway_bindings
        .into_iter()
        .filter(|b| {
            config.gateways.iter().any(|g| {
                g.resource_name == b.gateway_resource && g.api_type != config::ApiType::WebSocket
            })
        })
        .collect();
    let has_http_gateways = config
        .gateways
        .iter()
        .any(|g| g.api_type != config::ApiType::WebSocket);

    // Start HTTP server(s) (blocks until shutdown)
    if http_bindings.len() > 1 {
        server::start_multi_gateway(
            config,
            dir,
            http_bindings,
            watch,
            cors_config.as_ref(),
            debug_options,
        )
        .await?;
    } else if has_http_gateways {
        if watch {
            server::start_server_with_watch(config, dir, port, cors_config.as_ref(), debug_options)
                .await?;
        } else {
            server::start_server(config, dir, port, cors_config.as_ref(), debug_options).await?;
        }
    } else if !ws_handles.is_empty() {
        // Only WebSocket gateways, wait for them
        let (result, _, _) = futures::future::select_all(ws_handles).await;
        result??;
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
                let available: Vec<_> = tf_config
                    .functions
                    .iter()
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
                serde_json::from_str::<serde_json::Value>(&e).map_err(|e| {
                    anyhow::anyhow!(
                        "Invalid event JSON: {}\nHint: Make sure to quote your JSON properly.",
                        e
                    )
                })?;
                e
            }
            (None, Some(f)) => {
                let content = std::fs::read_to_string(&f).map_err(|e| {
                    anyhow::anyhow!("Cannot read event file '{}': {}", f.display(), e)
                })?;
                serde_json::from_str::<serde_json::Value>(&content)
                    .map_err(|e| anyhow::anyhow!("Invalid JSON in '{}': {}", f.display(), e))?;
                content
            }
            (None, None) => "{}".to_string(),
        };

        println!(
            "⚡ Invoking {} ({:?})",
            lambda.function_name, lambda.runtime
        );

        // Build a minimal Lambda event for direct invocation
        let lambda_event = runtime::LambdaEvent {
            http_method: "INVOKE".to_string(),
            path: "/".to_string(),
            resource: "/".to_string(),
            path_parameters: None,
            query_string_parameters: None,
            headers: None,
            body: Some(event_json),
            is_base64_encoded: false,
            request_context: runtime::RequestContext {
                stage: "local".to_string(),
                resource_path: "/".to_string(),
                http_method: "INVOKE".to_string(),
                request_id: format!(
                    "invoke-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ),
                api_id: "lambdaform".to_string(),
                path: "/".to_string(),
                identity: runtime::RequestIdentity {
                    source_ip: "127.0.0.1".to_string(),
                },
            },
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

fn cmd_config(json_output: bool, dir: PathBuf, var_files: Vec<PathBuf>) -> anyhow::Result<()> {
    let config = parser::parse_terraform_dir_with_var_files(&dir, &var_files)?;

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
                    println!(
                        "     Env vars: {:?}",
                        f.environment.keys().collect::<Vec<_>>()
                    );
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
                        println!(
                            "     {:?} {} → {}",
                            route.method, route.path, route.function_resource
                        );
                    }
                }
            }
        }

        if !config.state_machines.is_empty() {
            println!("\n🔄 Step Functions ({}):", config.state_machines.len());
            for sm in &config.state_machines {
                let summary = stepfunctions::summarize(&sm.definition)
                    .unwrap_or_else(|| "unparseable".to_string());
                println!("   • {} ({}) — {}", sm.name, sm.machine_type, summary);
            }
            println!("\n   Run `lambdaform stepfunctions` for full visualization.");
        }

        if !config.dynamodb_tables.is_empty() {
            println!("\n🗄️  DynamoDB Tables ({}):", config.dynamodb_tables.len());
            for table in &config.dynamodb_tables {
                let keys = match (&table.hash_key, &table.range_key) {
                    (Some(hk), Some(rk)) => format!("{} (PK) + {} (SK)", hk, rk),
                    (Some(hk), None) => format!("{} (PK)", hk),
                    _ => "unknown".to_string(),
                };
                println!("   • {} — {}", table.name, keys);
                if !table.gsi_names.is_empty() {
                    println!("     GSIs: {}", table.gsi_names.join(", "));
                }
                if !table.lsi_names.is_empty() {
                    println!("     LSIs: {}", table.lsi_names.join(", "));
                }
                if table.stream_enabled {
                    println!("     Streams: enabled");
                }
            }
            println!("\n   💡 Local development hint:");
            println!("      Run DynamoDB Local: docker run -p 8000:8000 amazon/dynamodb-local");
            println!("      Set env in lambdaform.yaml:");
            println!("        environment:");
            println!("          AWS_ENDPOINT_URL: http://localhost:8000");
            println!("          AWS_REGION: us-east-1");
            println!("          AWS_ACCESS_KEY_ID: local");
            println!("          AWS_SECRET_ACCESS_KEY: local");
        }

        if !config.sqs_queues.is_empty() {
            println!("\n📬 SQS Queues ({}):", config.sqs_queues.len());
            for q in &config.sqs_queues {
                let fifo = if q.fifo_queue { " (FIFO)" } else { "" };
                println!("   • {}{}", q.name, fifo);
            }
        }

        if !config.sns_topics.is_empty() {
            println!("\n📢 SNS Topics ({}):", config.sns_topics.len());
            for t in &config.sns_topics {
                let fifo = if t.fifo_topic { " (FIFO)" } else { "" };
                println!("   • {}{}", t.name, fifo);
            }
        }

        if !config.event_source_mappings.is_empty() {
            println!(
                "\n🔗 Event Source Mappings ({}):",
                config.event_source_mappings.len()
            );
            for esm in &config.event_source_mappings {
                let status = if esm.enabled { "enabled" } else { "disabled" };
                println!(
                    "   • {:?} {} → {} (batch: {}, {})",
                    esm.source_type,
                    esm.source_resource,
                    esm.function_resource,
                    esm.batch_size,
                    status
                );
            }
            println!(
                "\n   💡 Trigger: lambdaform trigger -t sqs -s <queue> -m '{{\"key\":\"value\"}}'"
            );
        }
    }

    Ok(())
}

fn cmd_validate(dir: PathBuf, var_files: Vec<PathBuf>) -> anyhow::Result<()> {
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
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "tf"))
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
    let config = match parser::parse_terraform_dir_with_var_files(&dir, &var_files) {
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
            config::Runtime::Go1
            | config::Runtime::ProvidedAl2
            | config::Runtime::ProvidedAl2023 => {
                warnings.push(format!(
                    "Function '{}': runtime {:?} is not yet supported (coming soon)",
                    f.function_name, f.runtime
                ));
            }
            _ => {}
        }

        // Check handler file exists
        let (file, _func) = f
            .handler
            .rsplitn(2, '.')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .windows(2)
            .next()
            .map(|w| (w[0], w[1]))
            .unwrap_or(("", ""));

        if !file.is_empty() {
            let ext = if f.runtime.is_nodejs() {
                "js"
            } else if f.runtime.is_python() {
                "py"
            } else {
                ""
            };
            if !ext.is_empty() {
                let filename = format!("{}.{}", file, ext);
                let found = ["", "src/", "lib/", "lambda/"]
                    .iter()
                    .any(|prefix| dir.join(format!("{}{}", prefix, filename)).exists());
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
    let fn_names: Vec<&str> = config
        .functions
        .iter()
        .map(|f| f.resource_name.as_str())
        .collect();
    for gw in &config.gateways {
        for route in &gw.routes {
            if !fn_names.contains(&route.function_resource.as_str()) {
                errors.push(format!(
                    "Route {} {} → '{}': function not found (available: {})",
                    route.method_str(),
                    route.path,
                    route.function_resource,
                    fn_names.join(", ")
                ));
            }
        }
    }

    // Print results
    println!(
        "   Found {} function(s), {} gateway(s), {} route(s)",
        config.functions.len(),
        config.gateways.len(),
        config
            .gateways
            .iter()
            .map(|g| g.routes.len())
            .sum::<usize>()
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

async fn cmd_trigger(
    source_type: String,
    source: String,
    message: String,
    batch: u32,
    dir: PathBuf,
    function: Option<String>,
) -> anyhow::Result<()> {
    println!("📨 Lambdaform Trigger Simulation\n");

    let mut config = parser::parse_terraform_dir(&dir)?;

    // Apply project config
    let project_config = project_config::ProjectConfig::load(&dir)?;
    if let Some(ref pc) = project_config {
        pc.apply(&mut config);
    }

    // Build message batch
    let messages: Vec<String> = (0..batch).map(|_| message.clone()).collect();

    // If --function is provided, bypass event source mapping lookup and invoke directly
    if let Some(ref fn_name) = function {
        let lambda = config
            .functions
            .iter()
            .find(|f| f.resource_name == *fn_name || f.function_name == *fn_name)
            .ok_or_else(|| {
                let available: Vec<_> = config
                    .functions
                    .iter()
                    .map(|f| format!("  • {}", f.resource_name))
                    .collect();
                anyhow::anyhow!(
                    "Function '{}' not found.\n\nAvailable:\n{}",
                    fn_name,
                    available.join("\n")
                )
            })?;

        let event_payload = match source_type.as_str() {
            "sqs" => {
                let queue = config
                    .sqs_queues
                    .iter()
                    .find(|q| q.resource_name == source || q.name == source);
                let (name, fifo) = queue
                    .map(|q| (q.name.clone(), q.fifo_queue))
                    .unwrap_or((source.clone(), false));
                trigger::build_sqs_event(&name, &messages, fifo)
            }
            "sns" => {
                let topic = config
                    .sns_topics
                    .iter()
                    .find(|t| t.resource_name == source || t.name == source);
                let name = topic.map(|t| t.name.clone()).unwrap_or(source.clone());
                trigger::build_sns_event(&name, &messages)
            }
            _ => anyhow::bail!(
                "Unsupported trigger type '{}'. Use 'sqs' or 'sns'.",
                source_type
            ),
        };

        println!(
            "⚡ Triggering {} → {} with {} message(s)",
            source, lambda.function_name, batch
        );

        let executor = runtime::FunctionExecutor::new(lambda.clone(), dir);
        match executor.invoke_raw_event(event_payload).await {
            Ok(result) => {
                println!("✅ Success");
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            Err(e) => println!("❌ Failed: {}", e),
        }
        return Ok(());
    }

    // Standard path: use event source mapping
    trigger::execute_trigger(&config, &source_type, &source, messages, &dir).await
}

fn cmd_stepfunctions(dir: PathBuf, name: Option<String>, json_output: bool) -> anyhow::Result<()> {
    let config = parser::parse_terraform_dir(&dir)?;

    if config.state_machines.is_empty() {
        println!(
            "⚠️  No Step Functions state machines found in {}",
            dir.display()
        );
        println!("\nHint: Make sure your .tf files contain aws_sfn_state_machine resources.");
        return Ok(());
    }

    let machines: Vec<_> = if let Some(ref filter) = name {
        config
            .state_machines
            .iter()
            .filter(|sm| sm.name == *filter || sm.resource_name == *filter)
            .collect()
    } else {
        config.state_machines.iter().collect()
    };

    if machines.is_empty() {
        let available: Vec<_> = config
            .state_machines
            .iter()
            .map(|sm| format!("  • {} ({})", sm.name, sm.resource_name))
            .collect();
        anyhow::bail!(
            "State machine '{}' not found.\n\nAvailable:\n{}",
            name.unwrap_or_default(),
            available.join("\n")
        );
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&config.state_machines)?);
        return Ok(());
    }

    println!("📂 Step Functions in: {}\n", dir.display());
    println!("Found {} state machine(s)\n", machines.len());

    for sm in &machines {
        println!("{}", "═".repeat(60));
        println!(
            "{}",
            stepfunctions::render_ascii(&sm.name, &sm.machine_type, &sm.definition)
        );

        if let Some(summary) = stepfunctions::summarize(&sm.definition) {
            println!("   Summary: {}", summary);
        }
        println!();
    }

    Ok(())
}
