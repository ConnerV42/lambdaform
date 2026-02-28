//! Infrastructure graph visualization
//!
//! Renders Lambda→APIGW→DynamoDB→SQS→SNS relationships as DOT or ASCII.

use crate::config::{ApiType, EventSourceType, LambdaformConfig};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Node in the infrastructure graph
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeKind {
    Lambda,
    ApiGateway,
    DynamoDB,
    SqsQueue,
    SnsTopic,
    Layer,
    StepFunction,
}

impl NodeKind {
    fn label(&self) -> &str {
        match self {
            NodeKind::Lambda => "λ Lambda",
            NodeKind::ApiGateway => "⬡ API Gateway",
            NodeKind::DynamoDB => "⊞ DynamoDB",
            NodeKind::SqsQueue => "⇉ SQS",
            NodeKind::SnsTopic => "⊕ SNS",
            NodeKind::Layer => "▤ Layer",
            NodeKind::StepFunction => "⟳ Step Functions",
        }
    }

    fn dot_color(&self) -> &str {
        match self {
            NodeKind::Lambda => "#FF9900",
            NodeKind::ApiGateway => "#6B48FF",
            NodeKind::DynamoDB => "#3366FF",
            NodeKind::SqsQueue => "#E7157B",
            NodeKind::Layer => "#00B4AB",
            NodeKind::SnsTopic => "#D63AFF",
            NodeKind::StepFunction => "#E25D22",
        }
    }

    fn dot_shape(&self) -> &str {
        match self {
            NodeKind::Lambda => "box",
            NodeKind::ApiGateway => "hexagon",
            NodeKind::DynamoDB => "cylinder",
            NodeKind::SqsQueue => "parallelogram",
            NodeKind::SnsTopic => "diamond",
            NodeKind::Layer => "component",
            NodeKind::StepFunction => "doubleoctagon",
        }
    }

    fn ansi_color(&self) -> &str {
        match self {
            NodeKind::Lambda => "\x1b[33m",       // yellow
            NodeKind::ApiGateway => "\x1b[35m",   // magenta
            NodeKind::DynamoDB => "\x1b[34m",     // blue
            NodeKind::SqsQueue => "\x1b[31m",     // red
            NodeKind::SnsTopic => "\x1b[95m",     // bright magenta
            NodeKind::Layer => "\x1b[36m",        // cyan
            NodeKind::StepFunction => "\x1b[91m", // bright red
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub display_name: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub label: String,
    pub style: EdgeStyle,
}

#[derive(Debug, Clone)]
pub enum EdgeStyle {
    Solid,
    Dashed,
}

/// Build the infrastructure graph from parsed config
pub fn build_graph(config: &LambdaformConfig) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Lambda functions
    for func in &config.functions {
        let id = format!("lambda_{}", func.resource_name);
        let mut details = vec![
            format!("runtime: {}", func.runtime.as_str()),
            format!("memory: {}MB, timeout: {}s", func.memory_size, func.timeout),
        ];
        if let Some(ref path) = func.source_path {
            details.push(format!("src: {}", path.display()));
        }
        nodes.push(GraphNode {
            id,
            kind: NodeKind::Lambda,
            display_name: func.function_name.clone(),
            details,
        });
    }

    // API Gateways + routes → edges to Lambda
    for gw in &config.gateways {
        let gw_id = format!("apigw_{}", gw.resource_name);
        let api_type_str = match gw.api_type {
            ApiType::Rest => "REST (v1)",
            ApiType::Http => "HTTP (v2)",
            ApiType::WebSocket => "WebSocket",
        };
        nodes.push(GraphNode {
            id: gw_id.clone(),
            kind: NodeKind::ApiGateway,
            display_name: gw.name.clone(),
            details: vec![
                format!("type: {api_type_str}"),
                format!("{} routes", gw.routes.len()),
            ],
        });

        for route in &gw.routes {
            let lambda_id = format!("lambda_{}", route.function_resource);
            edges.push(GraphEdge {
                from: gw_id.clone(),
                to: lambda_id,
                label: format!("{} {}", route.method_str(), route.path),
                style: EdgeStyle::Solid,
            });

            // Authorizer edge
            if let Some(ref auth) = route.authorizer {
                if let Some(ref func_res) = auth.function_resource {
                    edges.push(GraphEdge {
                        from: gw_id.clone(),
                        to: format!("lambda_{func_res}"),
                        label: "authorizer".to_string(),
                        style: EdgeStyle::Dashed,
                    });
                }
            }
        }
    }

    // DynamoDB tables
    for table in &config.dynamodb_tables {
        let id = format!("dynamodb_{}", table.resource_name);
        let mut details = vec![format!("billing: {}", table.billing_mode)];
        if let Some(ref hk) = table.hash_key {
            details.push(format!("pk: {hk}"));
        }
        if let Some(ref rk) = table.range_key {
            details.push(format!("sk: {rk}"));
        }
        if !table.gsi_names.is_empty() {
            details.push(format!("GSIs: {}", table.gsi_names.join(", ")));
        }
        nodes.push(GraphNode {
            id,
            kind: NodeKind::DynamoDB,
            display_name: table.name.clone(),
            details,
        });
    }

    // SQS queues
    for queue in &config.sqs_queues {
        let id = format!("sqs_{}", queue.resource_name);
        let mut details = vec![];
        if queue.fifo_queue {
            details.push("FIFO".to_string());
        }
        nodes.push(GraphNode {
            id,
            kind: NodeKind::SqsQueue,
            display_name: queue.name.clone(),
            details,
        });
    }

    // SNS topics
    for topic in &config.sns_topics {
        let id = format!("sns_{}", topic.resource_name);
        let mut details = vec![];
        if topic.fifo_topic {
            details.push("FIFO".to_string());
        }
        nodes.push(GraphNode {
            id,
            kind: NodeKind::SnsTopic,
            display_name: topic.name.clone(),
            details,
        });
    }

    // Lambda layers
    for layer in &config.layers {
        let id = format!("layer_{}", layer.resource_name);
        let details = if layer.compatible_runtimes.is_empty() {
            vec![]
        } else {
            vec![format!(
                "runtimes: {}",
                layer.compatible_runtimes.join(", ")
            )]
        };
        nodes.push(GraphNode {
            id,
            kind: NodeKind::Layer,
            display_name: layer.layer_name.clone(),
            details,
        });
    }

    // Step Functions
    for sfn in &config.state_machines {
        let id = format!("sfn_{}", sfn.resource_name);
        nodes.push(GraphNode {
            id,
            kind: NodeKind::StepFunction,
            display_name: sfn.name.clone(),
            details: vec![format!("type: {}", sfn.machine_type)],
        });
    }

    // Event source mappings → edges
    for esm in &config.event_source_mappings {
        let source_id = match esm.source_type {
            EventSourceType::Sqs => format!("sqs_{}", esm.source_resource),
            EventSourceType::Sns => format!("sns_{}", esm.source_resource),
            EventSourceType::DynamoDb => format!("dynamodb_{}", esm.source_resource),
            EventSourceType::Kinesis => format!("kinesis_{}", esm.source_resource),
        };
        let target_id = format!("lambda_{}", esm.function_resource);
        let label = match esm.source_type {
            EventSourceType::Sqs => format!("SQS trigger (batch {})", esm.batch_size),
            EventSourceType::Sns => "SNS subscription".to_string(),
            EventSourceType::DynamoDb => "DynamoDB stream".to_string(),
            EventSourceType::Kinesis => "Kinesis stream".to_string(),
        };
        edges.push(GraphEdge {
            from: source_id,
            to: target_id,
            label,
            style: EdgeStyle::Solid,
        });
    }

    // Layer edges (Lambda → Layer)
    for func in &config.functions {
        let func_id = format!("lambda_{}", func.resource_name);
        for layer_ref in &func.layers {
            edges.push(GraphEdge {
                from: func_id.clone(),
                to: format!("layer_{layer_ref}"),
                label: "uses layer".to_string(),
                style: EdgeStyle::Dashed,
            });
        }
    }

    // DynamoDB references from Lambda env vars (heuristic: TABLE_NAME-like vars)
    for func in &config.functions {
        let func_id = format!("lambda_{}", func.resource_name);
        for value in func.environment.values() {
            // Match env var values that reference DynamoDB table resource names
            for table in &config.dynamodb_tables {
                if value.contains(&table.resource_name) || value == &table.name {
                    edges.push(GraphEdge {
                        from: func_id.clone(),
                        to: format!("dynamodb_{}", table.resource_name),
                        label: "env ref".to_string(),
                        style: EdgeStyle::Dashed,
                    });
                }
            }
        }
    }

    // Step Functions → Lambda invocations (parse ASL for Lambda ARN references)
    for sfn in &config.state_machines {
        let sfn_id = format!("sfn_{}", sfn.resource_name);
        for func in &config.functions {
            if sfn.definition.contains(&func.resource_name) {
                edges.push(GraphEdge {
                    from: sfn_id.clone(),
                    to: format!("lambda_{}", func.resource_name),
                    label: "invokes".to_string(),
                    style: EdgeStyle::Solid,
                });
            }
        }
    }

    (nodes, edges)
}

/// Render the graph as Graphviz DOT format
pub fn render_dot(nodes: &[GraphNode], edges: &[GraphEdge]) -> String {
    let mut out = String::new();
    writeln!(out, "digraph lambdaform {{").unwrap();
    writeln!(out, "  rankdir=LR;").unwrap();
    writeln!(out, "  node [fontname=\"Helvetica\" fontsize=11];").unwrap();
    writeln!(out, "  edge [fontname=\"Helvetica\" fontsize=9];").unwrap();
    writeln!(out).unwrap();

    // Group nodes by kind for subgraph clustering
    let mut by_kind: BTreeMap<String, Vec<&GraphNode>> = BTreeMap::new();
    for node in nodes {
        by_kind
            .entry(format!("{:?}", node.kind))
            .or_default()
            .push(node);
    }

    for (kind_name, group) in &by_kind {
        if let Some(first) = group.first() {
            writeln!(out, "  subgraph cluster_{kind_name} {{").unwrap();
            writeln!(out, "    label=\"{}\";", first.kind.label()).unwrap();
            writeln!(out, "    style=dashed;").unwrap();
            writeln!(out, "    color=\"{}\";", first.kind.dot_color()).unwrap();
            for node in group {
                let detail = if node.details.is_empty() {
                    String::new()
                } else {
                    format!("\\n{}", node.details.join("\\n"))
                };
                writeln!(
                    out,
                    "    \"{}\" [label=\"{}{}\" shape={} style=filled fillcolor=\"{}20\" color=\"{}\"];",
                    node.id,
                    node.display_name,
                    detail,
                    node.kind.dot_shape(),
                    node.kind.dot_color(),
                    node.kind.dot_color(),
                ).unwrap();
            }
            writeln!(out, "  }}").unwrap();
        }
    }

    writeln!(out).unwrap();

    for edge in edges {
        let style = match edge.style {
            EdgeStyle::Solid => "solid",
            EdgeStyle::Dashed => "dashed",
        };
        writeln!(
            out,
            "  \"{}\" -> \"{}\" [label=\"{}\" style={style}];",
            edge.from, edge.to, edge.label,
        )
        .unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

/// Render the graph as colored ASCII art for terminal display
pub fn render_ascii(nodes: &[GraphNode], edges: &[GraphEdge]) -> String {
    let reset = "\x1b[0m";
    let bold = "\x1b[1m";
    let dim = "\x1b[2m";
    let mut out = String::new();

    writeln!(
        out,
        "{bold}╔══════════════════════════════════════════╗{reset}"
    )
    .unwrap();
    writeln!(
        out,
        "{bold}║     Lambdaform Infrastructure Graph      ║{reset}"
    )
    .unwrap();
    writeln!(
        out,
        "{bold}╚══════════════════════════════════════════╝{reset}"
    )
    .unwrap();
    writeln!(out).unwrap();

    // Group by kind
    let mut by_kind: BTreeMap<&str, Vec<&GraphNode>> = BTreeMap::new();
    for node in nodes {
        by_kind.entry(node.kind.label()).or_default().push(node);
    }

    // Build adjacency for showing connections inline
    let mut outgoing: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    let mut incoming: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for edge in edges {
        outgoing
            .entry(edge.from.as_str())
            .or_default()
            .push((edge.to.as_str(), edge.label.as_str()));
        incoming
            .entry(edge.to.as_str())
            .or_default()
            .push((edge.from.as_str(), edge.label.as_str()));
    }

    // Node id → display name map
    let name_map: BTreeMap<&str, &str> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.display_name.as_str()))
        .collect();

    for (kind_label, group) in &by_kind {
        let color = group[0].kind.ansi_color();
        writeln!(out, "{color}{bold}── {kind_label} ──{reset}").unwrap();

        for node in group {
            writeln!(out, "  {color}●{reset} {bold}{}{reset}", node.display_name).unwrap();
            for detail in &node.details {
                writeln!(out, "    {dim}{detail}{reset}").unwrap();
            }

            // Show outgoing edges
            if let Some(outs) = outgoing.get(node.id.as_str()) {
                for (target, label) in outs {
                    let target_name = name_map.get(target).unwrap_or(target);
                    writeln!(out, "    {dim}→ {target_name} ({label}){reset}").unwrap();
                }
            }
            // Show incoming edges
            if let Some(ins) = incoming.get(node.id.as_str()) {
                for (source, label) in ins {
                    let source_name = name_map.get(source).unwrap_or(source);
                    writeln!(out, "    {dim}← {source_name} ({label}){reset}").unwrap();
                }
            }
        }
        writeln!(out).unwrap();
    }

    // Summary
    let total_edges = edges.len();
    writeln!(
        out,
        "{dim}{} resources, {} connections{reset}",
        nodes.len(),
        total_edges,
    )
    .unwrap();

    out
}

/// Render as JSON for programmatic consumption
pub fn render_json(nodes: &[GraphNode], edges: &[GraphEdge]) -> serde_json::Value {
    let nodes_json: Vec<serde_json::Value> = nodes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "kind": format!("{:?}", n.kind),
                "name": n.display_name,
                "details": n.details,
            })
        })
        .collect();

    let edges_json: Vec<serde_json::Value> = edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "from": e.from,
                "to": e.to,
                "label": e.label,
                "style": format!("{:?}", e.style),
            })
        })
        .collect();

    serde_json::json!({
        "nodes": nodes_json,
        "edges": edges_json,
        "summary": {
            "total_resources": nodes.len(),
            "total_connections": edges.len(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use std::collections::HashMap;

    fn sample_config() -> LambdaformConfig {
        LambdaformConfig {
            functions: vec![
                LambdaConfig {
                    resource_name: "api_handler".to_string(),
                    function_name: "my-api-handler".to_string(),
                    handler: "index.handler".to_string(),
                    runtime: Runtime::Nodejs20,
                    source_path: Some("src/api".into()),
                    filename_ref: None,
                    environment: {
                        let mut m = HashMap::new();
                        m.insert("TABLE_NAME".to_string(), "users-table".to_string());
                        m
                    },
                    timeout: 30,
                    memory_size: 256,
                    layers: vec!["shared_utils".to_string()],
                    architecture: crate::config::Architecture::default(),
                },
                LambdaConfig {
                    resource_name: "worker".to_string(),
                    function_name: "my-worker".to_string(),
                    handler: "worker.handle".to_string(),
                    runtime: Runtime::Python312,
                    source_path: Some("src/worker".into()),
                    filename_ref: None,
                    environment: HashMap::new(),
                    timeout: 900,
                    memory_size: 512,
                    layers: vec![],
                    architecture: crate::config::Architecture::default(),
                },
            ],
            gateways: vec![ApiGatewayConfig {
                resource_name: "main_api".to_string(),
                name: "main-api".to_string(),
                api_type: ApiType::Http,
                routes: vec![
                    RouteConfig {
                        method: HttpMethod::Get,
                        path: "/users".to_string(),
                        function_resource: "api_handler".to_string(),
                        authorizer: None,
                    },
                    RouteConfig {
                        method: HttpMethod::Post,
                        path: "/users".to_string(),
                        function_resource: "api_handler".to_string(),
                        authorizer: None,
                    },
                ],
                route_selection_expression: None,
            }],
            dynamodb_tables: vec![DynamoDbTableConfig {
                resource_name: "users".to_string(),
                name: "users-table".to_string(),
                hash_key: Some("id".to_string()),
                range_key: None,
                billing_mode: "PAY_PER_REQUEST".to_string(),
                gsi_names: vec![],
                lsi_names: vec![],
                stream_enabled: false,
            }],
            sqs_queues: vec![SqsQueueConfig {
                resource_name: "work_queue".to_string(),
                name: "work-queue".to_string(),
                fifo_queue: false,
                visibility_timeout: 30,
            }],
            sns_topics: vec![],
            event_source_mappings: vec![EventSourceMappingConfig {
                resource_name: "worker_trigger".to_string(),
                source_type: EventSourceType::Sqs,
                source_resource: "work_queue".to_string(),
                function_resource: "worker".to_string(),
                batch_size: 10,
                enabled: true,
            }],
            layers: vec![LayerConfig {
                resource_name: "shared_utils".to_string(),
                layer_name: "shared-utils".to_string(),
                source_path: Some("layers/shared".into()),
                compatible_runtimes: vec!["nodejs20.x".to_string()],
            }],
            state_machines: vec![],
            archive_files: vec![],
            function_urls: vec![],
            detected_cors: None,
        }
    }

    #[test]
    fn test_build_graph_nodes() {
        let config = sample_config();
        let (nodes, _edges) = build_graph(&config);
        // 2 lambdas + 1 gateway + 1 dynamodb + 1 sqs + 1 layer = 6
        assert_eq!(nodes.len(), 6);
    }

    #[test]
    fn test_build_graph_edges() {
        let config = sample_config();
        let (_nodes, edges) = build_graph(&config);
        // 2 APIGW→Lambda routes + 1 SQS→Lambda ESM + 1 Lambda→Layer + 1 Lambda→DynamoDB env ref = 5
        assert_eq!(edges.len(), 5);
    }

    #[test]
    fn test_render_dot_output() {
        let config = sample_config();
        let (nodes, edges) = build_graph(&config);
        let dot = render_dot(&nodes, &edges);
        assert!(dot.starts_with("digraph lambdaform {"));
        assert!(dot.contains("my-api-handler"));
        assert!(dot.contains("main-api"));
        assert!(dot.contains("users-table"));
        assert!(dot.contains("work-queue"));
    }

    #[test]
    fn test_render_json_structure() {
        let config = sample_config();
        let (nodes, edges) = build_graph(&config);
        let json = render_json(&nodes, &edges);
        assert_eq!(json["summary"]["total_resources"], 6);
        assert_eq!(json["summary"]["total_connections"], 5);
        assert!(json["nodes"].is_array());
        assert!(json["edges"].is_array());
    }

    #[test]
    fn test_render_ascii_contains_resources() {
        let config = sample_config();
        let (nodes, edges) = build_graph(&config);
        let ascii = render_ascii(&nodes, &edges);
        assert!(ascii.contains("my-api-handler"));
        assert!(ascii.contains("main-api"));
        assert!(ascii.contains("6 resources, 5 connections"));
    }

    #[test]
    fn test_empty_config() {
        let config = LambdaformConfig::default();
        let (nodes, edges) = build_graph(&config);
        assert!(nodes.is_empty());
        assert!(edges.is_empty());
        let dot = render_dot(&nodes, &edges);
        assert!(dot.contains("digraph lambdaform"));
    }

    #[test]
    fn test_sns_topic_in_graph() {
        let mut config = LambdaformConfig::default();
        config.sns_topics.push(SnsTopicConfig {
            resource_name: "alerts".to_string(),
            name: "alert-topic".to_string(),
            fifo_topic: false,
        });
        config.functions.push(LambdaConfig {
            resource_name: "alert_handler".to_string(),
            function_name: "alert-handler".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 30,
            memory_size: 128,
            layers: vec![],
            architecture: Architecture::default(),
        });
        config.event_source_mappings.push(EventSourceMappingConfig {
            resource_name: "alert_sub".to_string(),
            source_type: EventSourceType::Sns,
            source_resource: "alerts".to_string(),
            function_resource: "alert_handler".to_string(),
            batch_size: 1,
            enabled: true,
        });
        let (nodes, edges) = build_graph(&config);
        assert_eq!(nodes.len(), 2); // lambda + sns
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].label, "SNS subscription");
    }

    #[test]
    fn test_step_function_invokes_lambda() {
        let mut config = LambdaformConfig::default();
        config.functions.push(LambdaConfig {
            resource_name: "processor".to_string(),
            function_name: "my-processor".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Python312,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 60,
            memory_size: 256,
            layers: vec![],
            architecture: Architecture::default(),
        });
        config.state_machines.push(StepFunctionConfig {
            resource_name: "pipeline".to_string(),
            name: "data-pipeline".to_string(),
            definition: r#"{"States":{"Process":{"Type":"Task","Resource":"arn:aws:lambda:us-east-1:123:function:processor"}}}"#.to_string(),
            machine_type: "STANDARD".to_string(),
            role_arn_ref: None,
        });
        let (nodes, edges) = build_graph(&config);
        assert_eq!(nodes.len(), 2); // lambda + sfn
                                    // sfn → lambda invocation edge
        let invoke_edges: Vec<_> = edges.iter().filter(|e| e.label == "invokes").collect();
        assert_eq!(invoke_edges.len(), 1);
        assert_eq!(invoke_edges[0].from, "sfn_pipeline");
        assert_eq!(invoke_edges[0].to, "lambda_processor");
    }

    #[test]
    fn test_authorizer_edge() {
        let mut config = LambdaformConfig::default();
        config.functions.push(LambdaConfig {
            resource_name: "api_fn".to_string(),
            function_name: "api".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 30,
            memory_size: 128,
            layers: vec![],
            architecture: Architecture::default(),
        });
        config.functions.push(LambdaConfig {
            resource_name: "auth_fn".to_string(),
            function_name: "authorizer".to_string(),
            handler: "auth.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 5,
            memory_size: 128,
            layers: vec![],
            architecture: Architecture::default(),
        });
        config.gateways.push(ApiGatewayConfig {
            resource_name: "gw".to_string(),
            name: "my-gw".to_string(),
            api_type: ApiType::Rest,
            routes: vec![RouteConfig {
                method: HttpMethod::Get,
                path: "/protected".to_string(),
                function_resource: "api_fn".to_string(),
                authorizer: Some(AuthorizerConfig {
                    auth_type: AuthorizerType::Lambda,
                    function_resource: Some("auth_fn".to_string()),
                }),
            }],
            route_selection_expression: None,
        });
        let (_nodes, edges) = build_graph(&config);
        let auth_edges: Vec<_> = edges.iter().filter(|e| e.label == "authorizer").collect();
        assert_eq!(auth_edges.len(), 1);
        assert_eq!(auth_edges[0].to, "lambda_auth_fn");
    }

    #[test]
    fn test_fifo_queue_detail() {
        let mut config = LambdaformConfig::default();
        config.sqs_queues.push(SqsQueueConfig {
            resource_name: "orders".to_string(),
            name: "orders.fifo".to_string(),
            fifo_queue: true,
            visibility_timeout: 60,
        });
        let (nodes, _edges) = build_graph(&config);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].details.contains(&"FIFO".to_string()));
    }

    #[test]
    fn test_node_kind_properties() {
        // Verify all NodeKind variants have non-empty labels, colors, shapes
        let kinds = vec![
            NodeKind::Lambda,
            NodeKind::ApiGateway,
            NodeKind::DynamoDB,
            NodeKind::SqsQueue,
            NodeKind::SnsTopic,
            NodeKind::Layer,
            NodeKind::StepFunction,
        ];
        for kind in &kinds {
            assert!(!kind.label().is_empty());
            assert!(kind.dot_color().starts_with('#'));
            assert!(!kind.dot_shape().is_empty());
            assert!(kind.ansi_color().starts_with("\x1b["));
        }
    }

    #[test]
    fn test_render_dot_edge_styles() {
        let nodes = vec![
            GraphNode {
                id: "a".into(),
                kind: NodeKind::Lambda,
                display_name: "A".into(),
                details: vec![],
            },
            GraphNode {
                id: "b".into(),
                kind: NodeKind::Layer,
                display_name: "B".into(),
                details: vec![],
            },
        ];
        let edges = vec![GraphEdge {
            from: "a".into(),
            to: "b".into(),
            label: "uses".into(),
            style: EdgeStyle::Dashed,
        }];
        let dot = render_dot(&nodes, &edges);
        assert!(dot.contains("style=dashed"));
    }

    #[test]
    fn test_render_json_node_fields() {
        let nodes = vec![GraphNode {
            id: "lambda_test".into(),
            kind: NodeKind::Lambda,
            display_name: "test-fn".into(),
            details: vec!["runtime: nodejs20.x".into()],
        }];
        let json = render_json(&nodes, &[]);
        let node = &json["nodes"][0];
        assert_eq!(node["id"], "lambda_test");
        assert_eq!(node["kind"], "Lambda");
        assert_eq!(node["name"], "test-fn");
        assert_eq!(node["details"][0], "runtime: nodejs20.x");
    }

    #[test]
    fn test_dynamodb_with_gsi_and_range_key() {
        let mut config = LambdaformConfig::default();
        config.dynamodb_tables.push(DynamoDbTableConfig {
            resource_name: "orders".to_string(),
            name: "orders-table".to_string(),
            hash_key: Some("orderId".to_string()),
            range_key: Some("timestamp".to_string()),
            billing_mode: "PAY_PER_REQUEST".to_string(),
            gsi_names: vec!["by-customer".to_string(), "by-status".to_string()],
            lsi_names: vec![],
            stream_enabled: false,
        });
        let (nodes, _edges) = build_graph(&config);
        assert_eq!(nodes.len(), 1);
        let details = &nodes[0].details;
        assert!(details.iter().any(|d| d.contains("orderId")));
        assert!(details.iter().any(|d| d.contains("timestamp")));
        assert!(details.iter().any(|d| d.contains("by-customer")));
    }

    #[test]
    fn test_dynamodb_stream_edge() {
        let mut config = LambdaformConfig::default();
        config.functions.push(LambdaConfig {
            resource_name: "stream_proc".to_string(),
            function_name: "stream-processor".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 30,
            memory_size: 128,
            layers: vec![],
            architecture: Architecture::default(),
        });
        config.dynamodb_tables.push(DynamoDbTableConfig {
            resource_name: "events".to_string(),
            name: "events-table".to_string(),
            hash_key: Some("pk".to_string()),
            range_key: Some("sk".to_string()),
            billing_mode: "PAY_PER_REQUEST".to_string(),
            gsi_names: vec![],
            lsi_names: vec![],
            stream_enabled: true,
        });
        config.event_source_mappings.push(EventSourceMappingConfig {
            resource_name: "stream_map".to_string(),
            source_type: EventSourceType::DynamoDb,
            source_resource: "events".to_string(),
            function_resource: "stream_proc".to_string(),
            batch_size: 100,
            enabled: true,
        });
        let (_nodes, edges) = build_graph(&config);
        let stream_edges: Vec<_> = edges
            .iter()
            .filter(|e| e.label == "DynamoDB stream")
            .collect();
        assert_eq!(stream_edges.len(), 1);
        assert_eq!(stream_edges[0].from, "dynamodb_events");
        assert_eq!(stream_edges[0].to, "lambda_stream_proc");
    }

    #[test]
    fn test_env_var_dynamodb_reference_edge() {
        let mut config = LambdaformConfig::default();
        config.functions.push(LambdaConfig {
            resource_name: "api".to_string(),
            function_name: "api-fn".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: {
                let mut m = HashMap::new();
                m.insert("TABLE".to_string(), "my-data-table".to_string());
                m.insert("OTHER".to_string(), "unrelated".to_string());
                m
            },
            timeout: 30,
            memory_size: 128,
            layers: vec![],
            architecture: Architecture::default(),
        });
        config.dynamodb_tables.push(DynamoDbTableConfig {
            resource_name: "data".to_string(),
            name: "my-data-table".to_string(),
            hash_key: Some("id".to_string()),
            range_key: None,
            billing_mode: "PAY_PER_REQUEST".to_string(),
            gsi_names: vec![],
            lsi_names: vec![],
            stream_enabled: false,
        });
        let (_nodes, edges) = build_graph(&config);
        let env_edges: Vec<_> = edges.iter().filter(|e| e.label == "env ref").collect();
        assert_eq!(env_edges.len(), 1);
        assert_eq!(env_edges[0].from, "lambda_api");
        assert_eq!(env_edges[0].to, "dynamodb_data");
    }

    #[test]
    fn test_multiple_layers_edges() {
        let mut config = LambdaformConfig::default();
        config.functions.push(LambdaConfig {
            resource_name: "fn1".to_string(),
            function_name: "fn1".to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 30,
            memory_size: 128,
            layers: vec!["layer_a".to_string(), "layer_b".to_string()],
            architecture: Architecture::default(),
        });
        config.layers.push(LayerConfig {
            resource_name: "layer_a".to_string(),
            layer_name: "Layer A".to_string(),
            source_path: None,
            compatible_runtimes: vec![],
        });
        config.layers.push(LayerConfig {
            resource_name: "layer_b".to_string(),
            layer_name: "Layer B".to_string(),
            source_path: None,
            compatible_runtimes: vec!["nodejs20.x".to_string(), "python3.12".to_string()],
        });
        let (nodes, edges) = build_graph(&config);
        assert_eq!(nodes.len(), 3); // 1 lambda + 2 layers
        let layer_edges: Vec<_> = edges.iter().filter(|e| e.label == "uses layer").collect();
        assert_eq!(layer_edges.len(), 2);
    }

    #[test]
    fn test_websocket_gateway_type_in_graph() {
        let mut config = LambdaformConfig::default();
        config.gateways.push(ApiGatewayConfig {
            resource_name: "ws".to_string(),
            name: "ws-api".to_string(),
            api_type: ApiType::WebSocket,
            routes: vec![],
            route_selection_expression: Some("$request.body.action".to_string()),
        });
        let (nodes, _edges) = build_graph(&config);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].details.iter().any(|d| d.contains("WebSocket")));
    }

    #[test]
    fn test_render_ascii_incoming_outgoing_arrows() {
        let nodes = vec![
            GraphNode {
                id: "sqs_q".into(),
                kind: NodeKind::SqsQueue,
                display_name: "my-queue".into(),
                details: vec![],
            },
            GraphNode {
                id: "lambda_w".into(),
                kind: NodeKind::Lambda,
                display_name: "worker".into(),
                details: vec!["runtime: nodejs20.x".into()],
            },
        ];
        let edges = vec![GraphEdge {
            from: "sqs_q".into(),
            to: "lambda_w".into(),
            label: "SQS trigger (batch 10)".into(),
            style: EdgeStyle::Solid,
        }];
        let ascii = render_ascii(&nodes, &edges);
        // Worker should show incoming arrow from queue
        assert!(ascii.contains("← my-queue (SQS trigger"));
        // Queue should show outgoing arrow to worker
        assert!(ascii.contains("→ worker (SQS trigger"));
    }

    #[test]
    fn test_render_json_edge_style() {
        let nodes = vec![];
        let edges = vec![
            GraphEdge {
                from: "a".into(),
                to: "b".into(),
                label: "solid-edge".into(),
                style: EdgeStyle::Solid,
            },
            GraphEdge {
                from: "c".into(),
                to: "d".into(),
                label: "dashed-edge".into(),
                style: EdgeStyle::Dashed,
            },
        ];
        let json = render_json(&nodes, &edges);
        assert_eq!(json["edges"][0]["style"], "Solid");
        assert_eq!(json["edges"][1]["style"], "Dashed");
    }

    #[test]
    fn test_render_dot_subgraph_clustering() {
        let config = sample_config();
        let (nodes, edges) = build_graph(&config);
        let dot = render_dot(&nodes, &edges);
        // Should have subgraph clusters for different resource types
        assert!(dot.contains("subgraph cluster_Lambda"));
        assert!(dot.contains("subgraph cluster_ApiGateway"));
        assert!(dot.contains("subgraph cluster_DynamoDB"));
        assert!(dot.contains("subgraph cluster_SqsQueue"));
        assert!(dot.contains("subgraph cluster_Layer"));
    }

    #[test]
    fn test_sns_fifo_detail() {
        let mut config = LambdaformConfig::default();
        config.sns_topics.push(SnsTopicConfig {
            resource_name: "orders".to_string(),
            name: "orders.fifo".to_string(),
            fifo_topic: true,
        });
        let (nodes, _edges) = build_graph(&config);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].details.contains(&"FIFO".to_string()));
    }

    #[test]
    fn test_layer_compatible_runtimes_in_details() {
        let mut config = LambdaformConfig::default();
        config.layers.push(LayerConfig {
            resource_name: "utils".to_string(),
            layer_name: "utils-layer".to_string(),
            source_path: None,
            compatible_runtimes: vec!["nodejs20.x".to_string(), "nodejs22.x".to_string()],
        });
        let (nodes, _edges) = build_graph(&config);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0]
            .details
            .iter()
            .any(|d| d.contains("nodejs20.x") && d.contains("nodejs22.x")));
    }

    #[test]
    fn test_large_graph_resource_count() {
        let mut config = LambdaformConfig::default();
        // Create 10 functions, 3 gateways, 2 tables, 2 queues, 1 topic
        for i in 0..10 {
            config.functions.push(LambdaConfig {
                resource_name: format!("fn_{i}"),
                function_name: format!("function-{i}"),
                handler: "index.handler".to_string(),
                runtime: Runtime::Nodejs20,
                source_path: None,
                filename_ref: None,
                environment: HashMap::new(),
                timeout: 30,
                memory_size: 128,
                layers: vec![],
                architecture: Architecture::default(),
            });
        }
        for i in 0..3 {
            config.gateways.push(ApiGatewayConfig {
                resource_name: format!("gw_{i}"),
                name: format!("api-{i}"),
                api_type: ApiType::Http,
                routes: vec![],
                route_selection_expression: None,
            });
        }
        let (nodes, _edges) = build_graph(&config);
        assert_eq!(nodes.len(), 13);
        let json = render_json(&nodes, &_edges);
        assert_eq!(json["summary"]["total_resources"], 13);
    }
}
