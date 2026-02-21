//! Lambdaform - Terraform-native local Lambda emulator
//!
//! The only local Lambda tool that reads your Terraform.

use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;
use walkdir::WalkDir;

use bollard::Docker;
use lambdaform::config;
use lambdaform::graph;
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

        /// Output logs as structured JSON (for log aggregators)
        #[arg(long)]
        json_log: bool,

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

        /// Enable terminal UI with live request log (requires --features tui)
        #[arg(long)]
        tui: bool,
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

        /// Show the generated event payload without invoking the function
        #[arg(long)]
        dry_run: bool,
    },

    /// Initialize a new Lambdaform project (generates lambdaform.yaml)
    Init {
        /// Directory to initialize
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Accept all defaults without prompting
        #[arg(short, long)]
        yes: bool,
    },

    /// Show or replay request history from previous sessions
    Replay {
        /// Directory containing Terraform files (with .lambdaform/)
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Replay a specific request by index (1-based)
        #[arg(short = 'n', long)]
        index: Option<usize>,

        /// Replay all recorded requests sequentially
        #[arg(long)]
        all: bool,

        /// Show only the last N entries
        #[arg(short, long)]
        last: Option<usize>,

        /// Filter by HTTP method (GET, POST, etc.)
        #[arg(short, long)]
        method: Option<String>,

        /// Filter by path prefix
        #[arg(short, long)]
        path: Option<String>,

        /// Clear history file
        #[arg(long)]
        clear: bool,

        /// Target port for replay (default: from recorded entry)
        #[arg(long)]
        port: Option<u16>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List configured plugins and their capabilities
    Plugins {
        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Estimate AWS Lambda invocation costs from local usage
    Cost {
        /// Directory containing Terraform files (with .lambdaform/)
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Architecture for pricing (x86 or arm)
        #[arg(short, long, default_value = "x86")]
        arch: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Additional .tfvars files to load (like terraform -var-file)
        #[arg(long = "var-file", value_name = "FILE")]
        var_files: Vec<PathBuf>,
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

    /// Generate shell completion scripts (bash, zsh, fish, powershell)
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Visualize infrastructure relationships (Lambda→APIGW→DynamoDB→SQS→SNS)
    Graph {
        /// Directory containing Terraform files
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,

        /// Output format: ascii (default), dot (Graphviz), or json
        #[arg(short, long, default_value = "ascii")]
        format: String,

        /// Additional .tfvars files to load
        #[arg(long = "var-file", value_name = "FILE")]
        var_files: Vec<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging — verbose flag sets DEBUG level, json_log enables structured output
    let tui_mode = matches!(&cli.command, Commands::Start { tui: true, .. });
    let verbose = matches!(&cli.command, Commands::Start { verbose: true, .. });
    let json_log = matches!(&cli.command, Commands::Start { json_log: true, .. });
    let default_level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let env_filter =
        tracing_subscriber::EnvFilter::from_default_env().add_directive(default_level.into());

    if tui_mode {
        // In TUI mode, suppress normal logging (TUI takes over the terminal)
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::WARN.into()),
            )
            .with_writer(std::io::sink)
            .init();
    } else if json_log {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(false)
            .with_span_list(false)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    match cli.command {
        Commands::Start {
            port,
            dir,
            watch,
            verbose: _,
            json_log: _,
            debug,
            debug_port,
            debug_python,
            debug_python_port,
            var_files,
            tui,
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
                tui,
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
            dry_run,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_trigger(
                source_type,
                source,
                message,
                batch,
                dir,
                function,
                dry_run,
            ))
        }
        Commands::Plugins { dir, json } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_plugins(dir, json))
        }
        Commands::Cost {
            dir,
            arch,
            json,
            var_files,
        } => cmd_cost(dir, arch, json, var_files),
        Commands::StepFunctions { dir, name, json } => cmd_stepfunctions(dir, name, json),
        Commands::Graph {
            dir,
            format,
            var_files,
        } => cmd_graph(dir, format, var_files),
        Commands::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                "lambdaform",
                &mut std::io::stdout(),
            );
            Ok(())
        }
        Commands::Init { dir, yes } => cmd_init(dir, yes),
        Commands::Replay {
            dir,
            index,
            all,
            last,
            method,
            path,
            clear,
            port,
            json,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_replay(
                dir, index, all, last, method, path, clear, port, json,
            ))
        }
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
    tui: bool,
) -> anyhow::Result<()> {
    // Check TUI feature availability
    #[cfg(not(feature = "tui"))]
    if tui {
        lambdaform::tui::tui_not_available();
    }
    #[cfg(not(feature = "tui"))]
    let _ = tui;
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

    // Load plugins if configured
    // Load plugins if configured
    if let Some(ref pc) = project_config {
        if !pc.plugins.is_empty() {
            println!("🔌 Loading {} plugin(s)...", pc.plugins.len());
            let pm = lambdaform::plugin::PluginManager::load_plugins(&pc.plugins, &dir).await?;
            println!("🔌 {} plugin(s) ready", pm.plugin_count());

            // Run on_resource hooks for all parsed resources
            // (Side effects like env vars get applied to function configs)
            let effects = run_plugin_resource_hooks(&pm, &config, &dir).await;
            apply_plugin_side_effects(&mut config, &effects);

            // Make plugin manager globally available for request/response hooks
            server::set_plugin_manager(pm);
        }
    }

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
    let mut has_java = false;
    for f in &config.functions {
        println!("   • {} ({:?}) → {}", f.function_name, f.runtime, f.handler);
        if f.runtime.is_java() {
            has_java = true;
        }
    }

    // Validate handler files exist at startup
    for f in &config.functions {
        if !f.handler.contains('.') {
            continue;
        }
        let parts: Vec<&str> = f.handler.rsplitn(2, '.').collect();
        if parts.len() < 2 {
            continue;
        }
        let file_part = parts[1]; // module path (e.g., "index" or "src/index")
        let ext = if f.runtime.is_nodejs() {
            Some("js")
        } else if f.runtime.is_python() {
            Some("py")
        } else {
            None
        };
        if let Some(ext) = ext {
            let source_dir = f.resolve_source_dir_with_archives(&dir, &config.archive_files);
            let handler_file = format!("{}.{}", file_part, ext);
            let full_path = source_dir.join(&handler_file);
            if !full_path.exists() {
                println!(
                    "   ⚠️  Function '{}': handler file '{}' not found in {}",
                    f.function_name,
                    handler_file,
                    source_dir.display()
                );
            }
        }
    }

    // Warn about Docker requirement for Java runtimes
    if has_java {
        let docker_ok = Docker::connect_with_local_defaults().is_ok();
        if docker_ok {
            println!("   🐳 Java functions detected — Docker will be used for invocation");
        } else {
            println!("   ⚠️  Java functions detected but Docker is not available!");
            println!("      Install Docker and start the daemon to invoke Java Lambdas.");
            println!("      Other runtimes (Node.js, Python, Go, Rust) will work normally.");
        }
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

    // Compute function URL bindings
    let function_url_bindings: Vec<server::FunctionUrlBinding> = config
        .function_urls
        .iter()
        .enumerate()
        .map(|(i, furl)| {
            // Function URLs get ports starting at base_port + 200
            let furl_port = port + 200 + i as u16;
            server::FunctionUrlBinding {
                function_resource: furl.function_resource.clone(),
                function_url_resource: furl.resource_name.clone(),
                port: furl_port,
                cors: furl.cors.clone(),
            }
        })
        .collect();

    // Log function URLs
    if !function_url_bindings.is_empty() {
        println!("\n🔗 Function URLs ({}):", function_url_bindings.len());
        for binding in &function_url_bindings {
            println!(
                "   📡 {} → http://localhost:{}",
                binding.function_resource, binding.port
            );
        }
    }

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
    } else if config.function_urls.is_empty() {
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

    // CORS config: explicit lambdaform.yaml > auto-detected from Terraform MOCK > permissive defaults
    let cors_config = project_config
        .as_ref()
        .and_then(|pc| pc.cors.clone())
        .or_else(|| config.detected_cors.clone());
    if project_config
        .as_ref()
        .and_then(|pc| pc.cors.as_ref())
        .is_some()
    {
        println!("🔓 CORS enabled via lambdaform.yaml");
    } else if config.detected_cors.is_some() {
        println!("🔓 CORS auto-configured from Terraform MOCK integration");
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

    // Set up TUI if enabled
    #[cfg(feature = "tui")]
    let tui_handle = if tui {
        let (tx, rx) = lambdaform::tui::create_tui_channel();
        server::set_tui_sender(tx);

        // Build server info for the TUI header
        let tui_ports: Vec<(String, u16)> = if http_bindings.len() > 1 {
            http_bindings
                .iter()
                .map(|b| (b.gateway_name.clone(), b.port))
                .collect()
        } else {
            vec![("".to_string(), port)]
        };
        let tui_functions: Vec<String> = config
            .functions
            .iter()
            .map(|f| f.function_name.clone())
            .collect();

        let server_info = lambdaform::tui::ui::ServerInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            ports: tui_ports,
            functions: tui_functions,
            watching: watch,
        };

        let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
        let shutdown_clone = shutdown.clone();

        // Spawn TUI on a blocking thread (it takes over the terminal)
        let handle = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(lambdaform::tui::ui::run_tui(
                rx,
                server_info,
                shutdown_clone,
            ))
        });

        Some((handle, shutdown))
    } else {
        None
    };

    // Start Function URL servers (each gets its own port)
    let mut furl_handles = Vec::new();
    for binding in &function_url_bindings {
        let furl_config = config.clone();
        let furl_dir = dir.clone();
        let furl_resource = binding.function_resource.clone();
        let furl_port = binding.port;
        let furl_cors = binding.cors.clone();
        let furl_debug = debug_options.clone();

        let handle = tokio::spawn(async move {
            let app = server::build_function_url_app(
                furl_config,
                furl_dir,
                furl_resource.clone(),
                furl_debug,
                furl_cors.as_ref(),
            );
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], furl_port));
            let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
                anyhow::anyhow!(
                    "Failed to bind Function URL for {} on port {}: {}",
                    furl_resource,
                    furl_port,
                    e
                )
            })?;
            tracing::info!(
                "🔗 Function URL for {} listening on http://{}",
                furl_resource,
                addr
            );
            axum::serve(listener, app)
                .await
                .map_err(|e| anyhow::anyhow!("Function URL server error: {}", e))
        });
        furl_handles.push(handle);
    }

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
    } else if !ws_handles.is_empty() || !furl_handles.is_empty() {
        // Only WebSocket gateways and/or Function URLs, wait for signal
        tokio::signal::ctrl_c().await?;
    }

    // Wait for TUI to clean up if it was running
    #[cfg(feature = "tui")]
    if let Some((handle, _shutdown)) = tui_handle {
        let _ = handle.await;
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
            multi_value_query_string_parameters: None,
            headers: None,
            multi_value_headers: None,
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

        let fn_dir = lambda.resolve_source_dir_with_archives(&dir, &tf_config.archive_files);
        let executor = runtime::FunctionExecutor::new(lambda.clone(), fn_dir);
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
    dry_run: bool,
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

        if dry_run {
            println!("🔍 Dry run — generated event payload:\n");
            println!("{}", serde_json::to_string_pretty(&event_payload)?);
            return Ok(());
        }

        println!(
            "⚡ Triggering {} → {} with {} message(s)",
            source, lambda.function_name, batch
        );

        let fn_dir = lambda.resolve_source_dir_with_archives(&dir, &config.archive_files);
        let executor = runtime::FunctionExecutor::new(lambda.clone(), fn_dir);
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
    if dry_run {
        // Build event for display without invoking
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
        println!("🔍 Dry run — generated event payload:\n");
        println!("{}", serde_json::to_string_pretty(&event_payload)?);
        return Ok(());
    }
    trigger::execute_trigger(&config, &source_type, &source, messages, &dir).await
}

async fn cmd_plugins(dir: PathBuf, json_output: bool) -> anyhow::Result<()> {
    // Load project config
    let pc = match project_config::ProjectConfig::load(&dir)? {
        Some(pc) => pc,
        None => {
            println!("No lambdaform.yaml found in {}", dir.display());
            println!("Run `lambdaform init` to create one.");
            return Ok(());
        }
    };

    if pc.plugins.is_empty() {
        println!("No plugins configured in lambdaform.yaml.");
        println!("\nAdd plugins to your lambdaform.yaml:");
        println!("  plugins:");
        println!("    - name: my-plugin");
        println!("      path: ./plugins/my-plugin.py");
        return Ok(());
    }

    println!("🔌 Loading {} plugin(s)...\n", pc.plugins.len());

    let pm = lambdaform::plugin::PluginManager::load_plugins(&pc.plugins, &dir).await?;

    if json_output {
        let info: Vec<serde_json::Value> = pc
            .plugins
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                serde_json::json!({
                    "name": entry.name,
                    "path": entry.path,
                    "config": entry.config,
                    "index": i + 1,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("{} plugin(s) loaded successfully:\n", pm.plugin_count());
        for (i, name) in pm.plugin_names().iter().enumerate() {
            println!("  {}. {}", i + 1, name);
        }
        println!("\nUse --json for detailed output.");
    }

    Ok(())
}

fn cmd_cost(
    dir: PathBuf,
    arch: String,
    json_output: bool,
    var_files: Vec<PathBuf>,
) -> anyhow::Result<()> {
    use lambdaform::cost;
    use lambdaform::history;

    let architecture = match arch.to_lowercase().as_str() {
        "arm" | "arm64" | "graviton" => cost::Architecture::Arm64,
        _ => cost::Architecture::X86_64,
    };

    // Load history
    let history_path = dir.join(".lambdaform").join("history.jsonl");
    if !history_path.exists() {
        println!("💰 Lambda Cost Estimation\n");
        println!("  No request history found.");
        println!("  Run `lambdaform start` and make some requests first.");
        println!("  History is recorded to .lambdaform/history.jsonl\n");
        return Ok(());
    }

    let entries = history::load_history(&history_path)?;
    if entries.is_empty() {
        println!("💰 Lambda Cost Estimation\n");
        println!("  History file exists but contains no entries.");
        println!("  Make some requests to your local server first.\n");
        return Ok(());
    }

    // Parse Terraform to get function memory configs
    let config = parser::parse_terraform_dir_with_var_files(&dir, &var_files)
        .unwrap_or_else(|_| config::LambdaformConfig::default());

    let report = cost::estimate_costs(&entries, &config.functions, architecture);

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&cost::format_report_json(&report))?
        );
    } else {
        print!("{}", cost::format_report(&report));
    }

    Ok(())
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

fn cmd_init(dir: PathBuf, accept_defaults: bool) -> anyhow::Result<()> {
    use console::style;
    use dialoguer::{Confirm, Input, MultiSelect};
    use std::collections::HashSet;

    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let config_path = dir.join("lambdaform.yaml");

    println!("\n{}  Lambdaform Init\n", style("⚡").bold());

    // Check for existing config
    if config_path.exists() {
        if accept_defaults {
            println!(
                "{}  lambdaform.yaml already exists — overwriting (--yes mode)",
                style("⚠").yellow()
            );
        } else {
            let overwrite = Confirm::new()
                .with_prompt("lambdaform.yaml already exists. Overwrite?")
                .default(false)
                .interact()?;
            if !overwrite {
                println!("Aborted.");
                return Ok(());
            }
        }
    }

    // Detect project structure
    println!("{}  Scanning {} ...", style("🔍").bold(), dir.display());

    let tf_files: Vec<_> = WalkDir::new(&dir)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy();
            (name.ends_with(".tf") || name.ends_with(".tf.json"))
                && !e.path().components().any(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    s == ".terraform" || s == "node_modules" || s == ".git"
                })
        })
        .collect();

    if tf_files.is_empty() {
        println!(
            "\n{}  No .tf files found in {}",
            style("⚠").yellow(),
            dir.display()
        );
        println!("   Run this command in a directory with Terraform files,");
        println!("   or use --dir to point to one.\n");
        return Ok(());
    }

    println!(
        "   Found {} Terraform file(s)",
        style(tf_files.len()).cyan()
    );

    // Try parsing to detect functions and gateways
    let tf_dir = dir.clone();
    let parse_result = parser::parse_terraform_dir(&tf_dir);

    let mut detected_runtimes: HashSet<String> = HashSet::new();
    let mut function_count = 0;
    let mut gateway_count = 0;
    let mut has_websocket = false;
    let mut has_dynamodb = false;
    let mut has_sqs_sns = false;

    if let Ok(ref config) = parse_result {
        function_count = config.functions.len();
        gateway_count = config.gateways.len();

        for f in &config.functions {
            detected_runtimes.insert(f.runtime.as_str().to_string());
        }
        for gw in &config.gateways {
            if matches!(gw.api_type, config::ApiType::WebSocket) {
                has_websocket = true;
            }
        }
        has_dynamodb = !config.dynamodb_tables.is_empty();
        has_sqs_sns = !config.sqs_queues.is_empty() || !config.sns_topics.is_empty();
    }

    // Print detection summary
    println!(
        "   Found {} Lambda function(s)",
        style(function_count).cyan()
    );
    println!(
        "   Found {} API Gateway(s){}",
        style(gateway_count).cyan(),
        if has_websocket {
            " (including WebSocket)"
        } else {
            ""
        }
    );
    if !detected_runtimes.is_empty() {
        let mut runtimes: Vec<_> = detected_runtimes.iter().cloned().collect();
        runtimes.sort();
        println!("   Runtimes: {}", style(runtimes.join(", ")).cyan());
    }
    if has_dynamodb {
        println!("   DynamoDB tables detected");
    }
    if has_sqs_sns {
        println!("   SQS/SNS triggers detected");
    }

    if let Err(ref e) = parse_result {
        println!("\n{}  Parse warning: {}", style("⚠").yellow(), e);
        println!("   lambdaform.yaml will still be generated with defaults.\n");
    }

    println!();

    // Collect config values
    let port: u16 = if accept_defaults {
        3000
    } else {
        Input::new()
            .with_prompt("Server port")
            .default(3000u16)
            .interact_text()?
    };

    let watch: bool = if accept_defaults {
        true
    } else {
        Confirm::new()
            .with_prompt("Enable hot reload?")
            .default(true)
            .interact()?
    };

    // Ask about optional features
    let mut enable_debug_node = false;
    let mut enable_debug_python = false;
    let mut env_vars: Vec<(String, String)> = Vec::new();

    if !accept_defaults {
        let has_node = detected_runtimes.iter().any(|r| r.contains("nodejs"));
        let has_python = detected_runtimes.iter().any(|r| r.contains("python"));

        // Feature selection
        let mut feature_options = vec!["Add environment variables"];
        if has_node {
            feature_options.push("Enable Node.js debugger");
        }
        if has_python {
            feature_options.push("Enable Python debugger");
        }

        let features = MultiSelect::new()
            .with_prompt("Optional features (space to select, enter to confirm)")
            .items(&feature_options)
            .interact()?;

        for &idx in &features {
            match feature_options[idx] {
                "Add environment variables" => {
                    println!("  Enter environment variables (empty name to stop):");
                    loop {
                        let key: String = Input::new()
                            .with_prompt("  Variable name")
                            .allow_empty(true)
                            .interact_text()?;
                        if key.is_empty() {
                            break;
                        }
                        let val: String = Input::new()
                            .with_prompt(format!("  Value for {}", key))
                            .interact_text()?;
                        env_vars.push((key, val));
                    }
                }
                "Enable Node.js debugger" => enable_debug_node = true,
                "Enable Python debugger" => enable_debug_python = true,
                _ => {}
            }
        }
    }

    // Generate YAML
    let mut yaml = String::new();
    yaml.push_str("# Lambdaform project configuration\n");
    yaml.push_str("# Docs: https://github.com/ConnerV42/lambdaform#configuration\n\n");

    yaml.push_str(&format!("port: {}\n", port));
    yaml.push_str(&format!("watch: {}\n", watch));

    if !env_vars.is_empty() {
        yaml.push_str("\nenvironment:\n");
        for (k, v) in &env_vars {
            yaml.push_str(&format!("  {}: \"{}\"\n", k, v.replace('"', "\\\"")));
        }
    }

    if enable_debug_node || enable_debug_python {
        yaml.push_str("\ndebug:\n");
        if enable_debug_node {
            yaml.push_str("  nodejs: true\n");
        }
        if enable_debug_python {
            yaml.push_str("  python: true\n");
        }
    }

    // Add commented-out function override example if functions exist
    if function_count > 0 {
        yaml.push_str("\n# Per-function overrides (uncomment and customize):\n");
        yaml.push_str("# functions:\n");
        if let Ok(ref config) = parse_result {
            if let Some(f) = config.functions.first() {
                yaml.push_str(&format!("#   {}:\n", f.resource_name));
                yaml.push_str("#     environment:\n");
                yaml.push_str("#       MY_VAR: \"my-value\"\n");
                yaml.push_str("#     timeout: 30\n");
            }
        }
    }

    // Write the file
    std::fs::write(&config_path, &yaml)?;

    println!(
        "{}  Created {}",
        style("✅").green(),
        style(config_path.display()).bold()
    );
    println!("\n   Next steps:");
    println!("   → lambdaform start        Start the dev server");
    println!("   → lambdaform validate     Check your Terraform files");
    println!("   → lambdaform config       View parsed configuration\n");

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_replay(
    dir: PathBuf,
    index: Option<usize>,
    all: bool,
    last: Option<usize>,
    method_filter: Option<String>,
    path_filter: Option<String>,
    clear: bool,
    port_override: Option<u16>,
    json: bool,
) -> anyhow::Result<()> {
    use lambdaform::history;

    let history_path = dir.join(".lambdaform").join("history.jsonl");

    if clear {
        if history_path.exists() {
            std::fs::remove_file(&history_path)?;
            println!("🗑️  Cleared request history");
        } else {
            println!("No history file found");
        }
        return Ok(());
    }

    if !history_path.exists() {
        println!("No request history found at {}", history_path.display());
        println!("\nHistory is recorded automatically when you run `lambdaform start`.");
        println!("Try making some requests first, then use `lambdaform replay` to review them.");
        return Ok(());
    }

    let mut entries = history::load_history(&history_path)?;

    if entries.is_empty() {
        println!("History file is empty");
        return Ok(());
    }

    // Apply filters
    if let Some(ref m) = method_filter {
        let m_upper = m.to_uppercase();
        entries.retain(|e| e.method == m_upper);
    }
    if let Some(ref p) = path_filter {
        entries.retain(|e| e.path.starts_with(p.as_str()));
    }

    // Apply --last
    if let Some(n) = last {
        if entries.len() > n {
            entries = entries.split_off(entries.len() - n);
        }
    }

    // Replay a specific request
    if let Some(idx) = index {
        if idx == 0 || idx > entries.len() {
            anyhow::bail!("Index {} out of range (1-{})", idx, entries.len());
        }
        let entry = &entries[idx - 1];
        return replay_request(entry, port_override).await;
    }

    // Replay all
    if all {
        println!("🔁 Replaying {} requests...\n", entries.len());
        for (i, entry) in entries.iter().enumerate() {
            println!("--- Request {} ---", i + 1);
            if let Err(e) = replay_request(entry, port_override).await {
                println!("  ❌ Error: {}", e);
            }
            println!();
        }
        return Ok(());
    }

    // Default: list history
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        println!(
            "📋 Request History ({} entries) — {}\n",
            entries.len(),
            history_path.display()
        );
        for (i, entry) in entries.iter().enumerate() {
            println!("  {}", history::format_entry(entry, i + 1));
        }
        println!("\n  Replay a request:  lambdaform replay -n <index>");
        println!("  Replay all:        lambdaform replay --all");
        println!("  Filter by method:  lambdaform replay -m POST");
        println!("  Clear history:     lambdaform replay --clear");
    }

    Ok(())
}

async fn replay_request(
    entry: &lambdaform::history::HistoryEntry,
    port_override: Option<u16>,
) -> anyhow::Result<()> {
    use http_body_util::{BodyExt, Empty, Full};
    use hyper::body::Bytes;
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;

    let port = port_override.unwrap_or(entry.port);
    let query_str = entry
        .query
        .as_ref()
        .filter(|q| !q.is_empty())
        .map(|q| {
            format!(
                "?{}",
                q.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&")
            )
        })
        .unwrap_or_default();

    let url = format!("http://127.0.0.1:{}{}{}", port, entry.path, query_str);
    println!("  → {} {} (fn: {})", entry.method, url, entry.function);

    let start = std::time::Instant::now();

    let uri: hyper::Uri = url.parse()?;
    let method: hyper::Method = entry.method.parse()?;

    // Build request
    let mut builder = hyper::Request::builder().method(method).uri(uri);

    // Add headers (skip host, content-length — hyper handles these)
    if let Some(ref headers) = entry.headers {
        for (k, v) in headers {
            let lower = k.to_lowercase();
            if lower != "host" && lower != "content-length" {
                builder = builder.header(k.as_str(), v.as_str());
            }
        }
    }

    let client = Client::builder(TokioExecutor::new())
        .build_http::<http_body_util::Either<Full<Bytes>, Empty<Bytes>>>();

    let request = if let Some(ref body) = entry.body {
        builder.body(http_body_util::Either::Left(Full::new(Bytes::from(
            body.clone(),
        ))))?
    } else {
        builder.body(http_body_util::Either::Right(Empty::new()))?
    };

    match client.request(request).await {
        Ok(response) => {
            let status = response.status().as_u16();
            let body_bytes = response
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .unwrap_or_default();
            let duration = start.elapsed();
            let time_ms = duration.as_secs_f64() * 1000.0;

            let body = String::from_utf8_lossy(&body_bytes);

            let icon = if (200..300).contains(&status) {
                "✅"
            } else if (400..500).contains(&status) {
                "⚠️"
            } else {
                "❌"
            };

            let display_body = if body.len() > 200 {
                format!("{}...", &body[..200])
            } else {
                body.to_string()
            };
            println!(
                "  ← {} {} [{:.0}ms] {}",
                icon, status, time_ms, display_body
            );
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("Connection refused") || err_str.contains("tcp connect error") {
                println!(
                    "  ← ❌ Connection refused (is `lambdaform start` running on port {}?)",
                    port
                );
            } else {
                println!("  ← ❌ Request failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Run on_resource hooks for all known resource types in the parsed config.
/// Returns accumulated side effects.
async fn run_plugin_resource_hooks(
    pm: &lambdaform::plugin::PluginManager,
    tf_config: &config::LambdaformConfig,
    _dir: &std::path::Path,
) -> Vec<lambdaform::plugin::PluginSideEffect> {
    let mut all_effects = Vec::new();

    // DynamoDB tables
    for table in &tf_config.dynamodb_tables {
        let attrs = serde_json::json!({
            "name": table.name,
            "hash_key": table.hash_key,
            "range_key": table.range_key,
            "billing_mode": table.billing_mode,
        });
        if let Ok(effects) = pm
            .on_resource("aws_dynamodb_table", &table.resource_name, attrs)
            .await
        {
            all_effects.extend(effects);
        }
    }

    // SQS queues
    for queue in &tf_config.sqs_queues {
        let attrs = serde_json::json!({
            "name": queue.name,
        });
        if let Ok(effects) = pm
            .on_resource("aws_sqs_queue", &queue.resource_name, attrs)
            .await
        {
            all_effects.extend(effects);
        }
    }

    // SNS topics
    for topic in &tf_config.sns_topics {
        let attrs = serde_json::json!({
            "name": topic.name,
        });
        if let Ok(effects) = pm
            .on_resource("aws_sns_topic", &topic.resource_name, attrs)
            .await
        {
            all_effects.extend(effects);
        }
    }

    all_effects
}

/// Apply plugin side effects (env vars, etc.) to the Lambda config.
fn apply_plugin_side_effects(
    tf_config: &mut config::LambdaformConfig,
    effects: &[lambdaform::plugin::PluginSideEffect],
) {
    for effect in effects {
        match effect {
            lambdaform::plugin::PluginSideEffect::EnvVar {
                functions,
                key,
                value,
            } => {
                for func in &mut tf_config.functions {
                    if functions.is_empty()
                        || functions
                            .iter()
                            .any(|f| f == &func.resource_name || f == &func.function_name)
                    {
                        func.environment.insert(key.clone(), value.clone());
                    }
                }
            }
            lambdaform::plugin::PluginSideEffect::Endpoint { service, url } => {
                tracing::info!("🔌 Plugin endpoint: {} → {}", service, url);
            }
            lambdaform::plugin::PluginSideEffect::Log { .. } => {
                // Already logged during on_resource
            }
        }
    }
}

fn cmd_graph(dir: PathBuf, format: String, var_files: Vec<PathBuf>) -> anyhow::Result<()> {
    let config = if var_files.is_empty() {
        parser::parse_terraform_dir(&dir)?
    } else {
        parser::parse_terraform_dir_with_var_files(&dir, &var_files)?
    };

    let (nodes, edges) = graph::build_graph(&config);

    if nodes.is_empty() {
        println!("No infrastructure resources found in {}", dir.display());
        return Ok(());
    }

    match format.as_str() {
        "dot" | "graphviz" => {
            print!("{}", graph::render_dot(&nodes, &edges));
        }
        "json" => {
            let json = graph::render_json(&nodes, &edges);
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        _ => {
            print!("{}", graph::render_ascii(&nodes, &edges));
        }
    }

    Ok(())
}
