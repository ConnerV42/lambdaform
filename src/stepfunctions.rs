//! Step Functions visualization (read-only)
//!
//! Parses Amazon States Language (ASL) definitions and renders
//! ASCII flow diagrams of state machine workflows.

use serde::Deserialize;
use std::collections::HashMap;

/// ASL State Machine definition
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StateMachineDefinition {
    #[serde(default)]
    pub comment: Option<String>,
    pub start_at: String,
    pub states: HashMap<String, State>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

/// ASL State
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
pub struct State {
    #[serde(rename = "Type")]
    pub state_type: String,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub end: Option<bool>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub seconds: Option<u64>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub branches: Option<Vec<StateMachineDefinition>>,
    #[serde(default)]
    pub choices: Option<Vec<Choice>>,
    #[serde(default, rename = "Default")]
    pub default_state: Option<String>,
    #[serde(default)]
    pub iterator: Option<Box<StateMachineDefinition>>,
    #[serde(default, rename = "Catch")]
    pub catch: Option<Vec<Catcher>>,
    #[serde(default, rename = "Retry")]
    pub retry: Option<Vec<Retrier>>,
    #[serde(default)]
    pub cause: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
}

/// Choice rule in a Choice state
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Choice {
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub variable: Option<String>,
    // Simplified — just capture Next for visualization
}

/// Catch clause
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Catcher {
    #[serde(default)]
    pub error_equals: Vec<String>,
    #[serde(default)]
    pub next: Option<String>,
}

/// Retry clause
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Retrier {
    #[serde(default)]
    pub error_equals: Vec<String>,
    #[serde(default)]
    pub max_attempts: Option<u32>,
}

/// Render a state machine definition as an ASCII flow diagram
pub fn render_ascii(name: &str, machine_type: &str, definition_json: &str) -> String {
    let mut output = String::new();
    
    let def: StateMachineDefinition = match serde_json::from_str(definition_json) {
        Ok(d) => d,
        Err(e) => {
            output.push_str(&format!("⚠️  Could not parse ASL definition: {}\n", e));
            output.push_str("   Raw definition:\n");
            // Show first 500 chars
            let preview: String = definition_json.chars().take(500).collect();
            output.push_str(&format!("   {}\n", preview));
            return output;
        }
    };
    
    // Header
    let type_icon = if machine_type == "EXPRESS" { "⚡" } else { "🔄" };
    output.push_str(&format!("{} {} ({})\n", type_icon, name, machine_type));
    if let Some(ref comment) = def.comment {
        output.push_str(&format!("   {}\n", comment));
    }
    if let Some(timeout) = def.timeout_seconds {
        output.push_str(&format!("   Timeout: {}s\n", timeout));
    }
    output.push('\n');
    
    // Walk the state machine
    render_flow(&def, &mut output, 0);
    
    output
}

fn render_flow(def: &StateMachineDefinition, output: &mut String, indent: usize) {
    let pad = "   ".repeat(indent);
    
    // Start
    output.push_str(&format!("{}  ┌─────────┐\n", pad));
    output.push_str(&format!("{}  │  START  │\n", pad));
    output.push_str(&format!("{}  └────┬────┘\n", pad));
    output.push_str(&format!("{}       │\n", pad));
    
    // Walk states in order starting from start_at
    let mut current = Some(def.start_at.clone());
    let mut visited = Vec::new();
    
    while let Some(state_name) = current {
        if visited.contains(&state_name) {
            output.push_str(&format!("{}       │\n", pad));
            output.push_str(&format!("{}  ↺ Loop back to: {}\n", pad, state_name));
            break;
        }
        visited.push(state_name.clone());
        
        if let Some(state) = def.states.get(&state_name) {
            render_state(&state_name, state, output, &pad);
            
            // Handle transitions
            match state.state_type.as_str() {
                "Choice" => {
                    // Show all branches
                    if let Some(ref choices) = state.choices {
                        for (i, choice) in choices.iter().enumerate() {
                            if let Some(ref next) = choice.next {
                                let var_hint = choice.variable.as_deref().unwrap_or("condition");
                                output.push_str(&format!("{}       ├── {} #{}: → {}\n", pad, var_hint, i + 1, next));
                            }
                        }
                    }
                    if let Some(ref default) = state.default_state {
                        output.push_str(&format!("{}       └── default: → {}\n", pad, default));
                    }
                    // Can't follow linear path through Choice — show all targets
                    current = None;
                    
                    // Render reachable states not yet visited
                    let mut targets: Vec<String> = Vec::new();
                    if let Some(ref choices) = state.choices {
                        for c in choices {
                            if let Some(ref n) = c.next {
                                if !visited.contains(n) && !targets.contains(n) {
                                    targets.push(n.clone());
                                }
                            }
                        }
                    }
                    if let Some(ref d) = state.default_state {
                        if !visited.contains(d) && !targets.contains(d) {
                            targets.push(d.clone());
                        }
                    }
                    
                    for target in targets {
                        output.push_str(&format!("\n{}  ── Branch: {} ──\n", pad, target));
                        let mut branch_current = Some(target);
                        while let Some(ref bn) = branch_current {
                            if visited.contains(bn) {
                                output.push_str(&format!("{}       → {} (already shown)\n", pad, bn));
                                break;
                            }
                            visited.push(bn.clone());
                            if let Some(bs) = def.states.get(bn.as_str()) {
                                render_state(bn, bs, output, &pad);
                                if bs.end.unwrap_or(false) {
                                    output.push_str(&format!("{}  ┌─────────┐\n", pad));
                                    output.push_str(&format!("{}  │   END   │\n", pad));
                                    output.push_str(&format!("{}  └─────────┘\n", pad));
                                    branch_current = None;
                                } else {
                                    branch_current = bs.next.clone();
                                }
                            } else {
                                branch_current = None;
                            }
                        }
                    }
                }
                "Parallel" => {
                    if let Some(ref branches) = state.branches {
                        for (i, branch) in branches.iter().enumerate() {
                            output.push_str(&format!("\n{}  ── Parallel Branch {} ──\n", pad, i + 1));
                            render_flow(branch, output, indent + 2);
                        }
                        output.push_str(&format!("\n{}  ── End Parallel ──\n", pad));
                    }
                    if state.end.unwrap_or(false) {
                        current = None;
                    } else {
                        current = state.next.clone();
                    }
                }
                "Map" => {
                    if let Some(ref iterator) = state.iterator {
                        output.push_str(&format!("\n{}  ── Map Iterator ──\n", pad));
                        render_flow(iterator, output, indent + 2);
                        output.push_str(&format!("\n{}  ── End Map ──\n", pad));
                    }
                    if state.end.unwrap_or(false) {
                        current = None;
                    } else {
                        current = state.next.clone();
                    }
                }
                _ => {
                    if state.end.unwrap_or(false) {
                        current = None;
                    } else {
                        current = state.next.clone();
                    }
                }
            }
            
            // Show end if terminal
            if state.end.unwrap_or(false) && state.state_type != "Choice" {
                output.push_str(&format!("{}  ┌─────────┐\n", pad));
                output.push_str(&format!("{}  │   END   │\n", pad));
                output.push_str(&format!("{}  └─────────┘\n", pad));
            } else if current.is_some() {
                output.push_str(&format!("{}       │\n", pad));
            }
        } else {
            output.push_str(&format!("{}  ⚠️  State '{}' not found in definition\n", pad, state_name));
            current = None;
        }
    }
}

fn render_state(name: &str, state: &State, output: &mut String, pad: &str) {
    let icon = match state.state_type.as_str() {
        "Task" => "⚙️",
        "Choice" => "◇",
        "Wait" => "⏳",
        "Pass" => "→",
        "Parallel" => "∥",
        "Map" => "🔁",
        "Succeed" => "✅",
        "Fail" => "❌",
        _ => "?",
    };
    
    // Build the box
    let label = format!("{} {} [{}]", icon, name, state.state_type);
    let width = label.len().max(20);
    let border = "─".repeat(width + 2);
    
    output.push_str(&format!("{}  ┌{}┐\n", pad, border));
    output.push_str(&format!("{}  │ {:<width$} │\n", pad, label, width = width));
    
    // Add details
    if let Some(ref resource) = state.resource {
        // Shorten ARN-like refs
        let short = if resource.contains("lambda") {
            resource.split(':').last().unwrap_or(resource)
        } else {
            resource
        };
        output.push_str(&format!("{}  │ {:<width$} │\n", pad, format!("  → {}", short), width = width));
    }
    if let Some(secs) = state.seconds {
        output.push_str(&format!("{}  │ {:<width$} │\n", pad, format!("  {}s wait", secs), width = width));
    }
    if let Some(ref catch) = state.catch {
        for c in catch {
            let errors = c.error_equals.join(", ");
            let next = c.next.as_deref().unwrap_or("?");
            output.push_str(&format!("{}  │ {:<width$} │\n", pad, format!("  catch [{}] → {}", errors, next), width = width));
        }
    }
    if let Some(ref retry) = state.retry {
        for r in retry {
            let errors = r.error_equals.join(", ");
            let attempts = r.max_attempts.unwrap_or(3);
            output.push_str(&format!("{}  │ {:<width$} │\n", pad, format!("  retry [{}] ×{}", errors, attempts), width = width));
        }
    }
    if let Some(ref comment) = state.comment {
        let short: String = comment.chars().take(width - 2).collect();
        output.push_str(&format!("{}  │ {:<width$} │\n", pad, format!("  # {}", short), width = width));
    }
    
    output.push_str(&format!("{}  └{}┘\n", pad, border));
}

/// Generate a summary of state machine statistics
pub fn summarize(definition_json: &str) -> Option<String> {
    let def: StateMachineDefinition = serde_json::from_str(definition_json).ok()?;
    
    let total = def.states.len();
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    for state in def.states.values() {
        *type_counts.entry(state.state_type.clone()).or_insert(0) += 1;
    }
    
    let mut parts = vec![format!("{} states", total)];
    for (t, count) in &type_counts {
        parts.push(format!("{} {}", count, t));
    }
    
    Some(parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    const SIMPLE_ASL: &str = r#"{
        "Comment": "A simple order workflow",
        "StartAt": "ValidateOrder",
        "States": {
            "ValidateOrder": {
                "Type": "Task",
                "Resource": "arn:aws:lambda:us-east-1:123:function:validate",
                "Next": "CheckInventory"
            },
            "CheckInventory": {
                "Type": "Task",
                "Resource": "arn:aws:lambda:us-east-1:123:function:check-inventory",
                "Next": "IsInStock"
            },
            "IsInStock": {
                "Type": "Choice",
                "Choices": [
                    {
                        "Variable": "$.inStock",
                        "BooleanEquals": true,
                        "Next": "ProcessPayment"
                    }
                ],
                "Default": "OutOfStock"
            },
            "ProcessPayment": {
                "Type": "Task",
                "Resource": "arn:aws:lambda:us-east-1:123:function:process-payment",
                "Retry": [
                    {
                        "ErrorEquals": ["States.TaskFailed"],
                        "MaxAttempts": 3
                    }
                ],
                "Catch": [
                    {
                        "ErrorEquals": ["PaymentFailed"],
                        "Next": "NotifyFailure"
                    }
                ],
                "Next": "ShipOrder"
            },
            "ShipOrder": {
                "Type": "Task",
                "Resource": "arn:aws:lambda:us-east-1:123:function:ship-order",
                "End": true
            },
            "OutOfStock": {
                "Type": "Task",
                "Resource": "arn:aws:lambda:us-east-1:123:function:notify-out-of-stock",
                "End": true
            },
            "NotifyFailure": {
                "Type": "Fail",
                "Cause": "Payment processing failed",
                "Error": "PaymentError"
            }
        }
    }"#;
    
    #[test]
    fn test_parse_simple_asl() {
        let def: StateMachineDefinition = serde_json::from_str(SIMPLE_ASL).unwrap();
        assert_eq!(def.start_at, "ValidateOrder");
        assert_eq!(def.states.len(), 7);
        assert_eq!(def.states["ValidateOrder"].state_type, "Task");
        assert_eq!(def.states["IsInStock"].state_type, "Choice");
    }
    
    #[test]
    fn test_render_ascii_simple() {
        let output = render_ascii("order-workflow", "STANDARD", SIMPLE_ASL);
        assert!(output.contains("order-workflow"));
        assert!(output.contains("START"));
        assert!(output.contains("ValidateOrder"));
        assert!(output.contains("CheckInventory"));
        assert!(output.contains("IsInStock"));
        assert!(output.contains("Choice"));
        assert!(output.contains("ProcessPayment"));
        assert!(output.contains("END"));
    }
    
    #[test]
    fn test_render_ascii_invalid_json() {
        let output = render_ascii("broken", "STANDARD", "not json");
        assert!(output.contains("Could not parse"));
    }
    
    #[test]
    fn test_summarize() {
        let summary = summarize(SIMPLE_ASL).unwrap();
        assert!(summary.contains("7 states"));
        assert!(summary.contains("Task"));
        assert!(summary.contains("Choice"));
        assert!(summary.contains("Fail"));
    }
    
    #[test]
    fn test_parallel_state() {
        let asl = r#"{
            "StartAt": "FanOut",
            "States": {
                "FanOut": {
                    "Type": "Parallel",
                    "Branches": [
                        {
                            "StartAt": "BranchA",
                            "States": {
                                "BranchA": { "Type": "Pass", "End": true }
                            }
                        },
                        {
                            "StartAt": "BranchB",
                            "States": {
                                "BranchB": { "Type": "Pass", "End": true }
                            }
                        }
                    ],
                    "End": true
                }
            }
        }"#;
        let output = render_ascii("parallel-test", "EXPRESS", asl);
        assert!(output.contains("Parallel Branch 1"));
        assert!(output.contains("Parallel Branch 2"));
        assert!(output.contains("BranchA"));
        assert!(output.contains("BranchB"));
        assert!(output.contains("⚡")); // EXPRESS icon
    }
    
    #[test]
    fn test_wait_state() {
        let asl = r#"{
            "StartAt": "WaitStep",
            "States": {
                "WaitStep": {
                    "Type": "Wait",
                    "Seconds": 30,
                    "Next": "Done"
                },
                "Done": { "Type": "Succeed" }
            }
        }"#;
        let output = render_ascii("wait-test", "STANDARD", asl);
        assert!(output.contains("30s wait"));
        assert!(output.contains("Succeed"));
    }
}
