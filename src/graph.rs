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
            EventSourceType::DynamoDb => format!("dynamodb_{}", esm.source_resource),
            EventSourceType::Kinesis => format!("kinesis_{}", esm.source_resource),
        };
        let target_id = format!("lambda_{}", esm.function_resource);
        let label = match esm.source_type {
            EventSourceType::Sqs => format!("SQS trigger (batch {})", esm.batch_size),
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
                    environment: {
                        let mut m = HashMap::new();
                        m.insert("TABLE_NAME".to_string(), "users-table".to_string());
                        m
                    },
                    timeout: 30,
                    memory_size: 256,
                    layers: vec!["shared_utils".to_string()],
                },
                LambdaConfig {
                    resource_name: "worker".to_string(),
                    function_name: "my-worker".to_string(),
                    handler: "worker.handle".to_string(),
                    runtime: Runtime::Python312,
                    source_path: Some("src/worker".into()),
                    environment: HashMap::new(),
                    timeout: 900,
                    memory_size: 512,
                    layers: vec![],
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
}
