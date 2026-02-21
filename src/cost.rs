//! Cost estimation for Lambda invocations
//!
//! Estimates AWS Lambda costs from local request history using AWS pricing:
//! - Requests: $0.20 per 1M requests
//! - Duration: $0.0000166667 per GB-second (x86)
//! - Duration: $0.0000133334 per GB-second (ARM/Graviton)
//!
//! Free tier: 1M requests + 400,000 GB-seconds per month.

use std::collections::HashMap;

use crate::config::LambdaConfig;
use crate::history::HistoryEntry;

/// Per-request cost in USD
const COST_PER_REQUEST: f64 = 0.0000002; // $0.20 per 1M

/// Cost per GB-second (x86_64)
const COST_PER_GB_SECOND_X86: f64 = 0.0000166667;

/// Cost per GB-second (ARM64/Graviton)
const COST_PER_GB_SECOND_ARM: f64 = 0.0000133334;

/// Free tier: requests per month
const FREE_TIER_REQUESTS: u64 = 1_000_000;

/// Free tier: GB-seconds per month
const FREE_TIER_GB_SECONDS: f64 = 400_000.0;

/// Re-export Architecture from config
pub use crate::config::Architecture;

/// Cost breakdown for a single function
#[derive(Debug, Clone)]
pub struct FunctionCost {
    pub function_name: String,
    pub resource_name: String,
    pub memory_mb: u32,
    pub invocations: u64,
    pub total_duration_ms: u64,
    pub avg_duration_ms: f64,
    pub p95_duration_ms: u64,
    pub max_duration_ms: u64,
    pub gb_seconds: f64,
    pub request_cost: f64,
    pub compute_cost: f64,
    pub total_cost: f64,
}

/// Aggregated cost report
#[derive(Debug, Clone)]
pub struct CostReport {
    pub functions: Vec<FunctionCost>,
    pub total_invocations: u64,
    pub total_gb_seconds: f64,
    pub total_request_cost: f64,
    pub total_compute_cost: f64,
    pub total_cost: f64,
    pub architecture: Architecture,
    /// Monthly projection based on observed rate
    pub monthly_projection: Option<MonthlyProjection>,
}

/// Monthly cost projection
#[derive(Debug, Clone)]
pub struct MonthlyProjection {
    pub projected_invocations: u64,
    pub projected_gb_seconds: f64,
    pub projected_cost_before_free_tier: f64,
    pub free_tier_savings: f64,
    pub projected_cost_after_free_tier: f64,
    pub observation_hours: f64,
}

/// Estimate costs from history entries + function configs
pub fn estimate_costs(
    entries: &[HistoryEntry],
    functions: &[LambdaConfig],
    architecture: Architecture,
) -> CostReport {
    if entries.is_empty() {
        return CostReport {
            functions: Vec::new(),
            total_invocations: 0,
            total_gb_seconds: 0.0,
            total_request_cost: 0.0,
            total_compute_cost: 0.0,
            total_cost: 0.0,
            architecture,
            monthly_projection: None,
        };
    }

    let cost_per_gb_second = match architecture {
        Architecture::X86_64 => COST_PER_GB_SECOND_X86,
        Architecture::Arm64 => COST_PER_GB_SECOND_ARM,
    };

    // Build function lookup by resource_name and function_name
    let fn_lookup: HashMap<&str, &LambdaConfig> = functions
        .iter()
        .flat_map(|f| vec![(f.resource_name.as_str(), f), (f.function_name.as_str(), f)])
        .collect();

    // Group entries by function
    let mut by_function: HashMap<String, Vec<&HistoryEntry>> = HashMap::new();
    for entry in entries {
        by_function
            .entry(entry.function.clone())
            .or_default()
            .push(entry);
    }

    let mut function_costs = Vec::new();

    for (fn_name, fn_entries) in &by_function {
        let memory_mb = fn_lookup
            .get(fn_name.as_str())
            .map(|f| f.memory_size)
            .unwrap_or(128); // Default Lambda memory

        let invocations = fn_entries.len() as u64;
        let mut durations: Vec<u64> = fn_entries.iter().map(|e| e.duration_ms).collect();
        durations.sort_unstable();

        let total_duration_ms: u64 = durations.iter().sum();
        let avg_duration_ms = total_duration_ms as f64 / invocations as f64;
        let p95_idx = ((durations.len() as f64 * 0.95) as usize).min(durations.len() - 1);
        let p95_duration_ms = durations[p95_idx];
        let max_duration_ms = *durations.last().unwrap_or(&0);

        // AWS bills in 1ms increments, minimum 1ms
        // GB-seconds = (memory_mb / 1024) * (duration_ms / 1000)
        let gb_seconds = (memory_mb as f64 / 1024.0) * (total_duration_ms as f64 / 1000.0);

        let request_cost = invocations as f64 * COST_PER_REQUEST;
        let compute_cost = gb_seconds * cost_per_gb_second;

        let resource = fn_lookup
            .get(fn_name.as_str())
            .map(|f| f.resource_name.clone())
            .unwrap_or_else(|| fn_name.clone());

        function_costs.push(FunctionCost {
            function_name: fn_name.clone(),
            resource_name: resource,
            memory_mb,
            invocations,
            total_duration_ms,
            avg_duration_ms,
            p95_duration_ms,
            max_duration_ms,
            gb_seconds,
            request_cost,
            compute_cost,
            total_cost: request_cost + compute_cost,
        });
    }

    // Sort by total cost descending
    function_costs.sort_by(|a, b| b.total_cost.partial_cmp(&a.total_cost).unwrap());

    let total_invocations: u64 = function_costs.iter().map(|f| f.invocations).sum();
    let total_gb_seconds: f64 = function_costs.iter().map(|f| f.gb_seconds).sum();
    let total_request_cost: f64 = function_costs.iter().map(|f| f.request_cost).sum();
    let total_compute_cost: f64 = function_costs.iter().map(|f| f.compute_cost).sum();
    let total_cost = total_request_cost + total_compute_cost;

    // Monthly projection: estimate from time span of history
    let monthly_projection = compute_monthly_projection(
        entries,
        total_invocations,
        total_gb_seconds,
        total_cost,
        cost_per_gb_second,
    );

    CostReport {
        functions: function_costs,
        total_invocations,
        total_gb_seconds,
        total_request_cost,
        total_compute_cost,
        total_cost,
        architecture,
        monthly_projection,
    }
}

fn compute_monthly_projection(
    entries: &[HistoryEntry],
    total_invocations: u64,
    total_gb_seconds: f64,
    _total_cost: f64,
    cost_per_gb_second: f64,
) -> Option<MonthlyProjection> {
    if entries.len() < 2 {
        return None;
    }

    // Parse timestamps to find time span
    let timestamps: Vec<&str> = entries.iter().map(|e| e.timestamp.as_str()).collect();
    let first = timestamps.first()?;
    let last = timestamps.last()?;

    // Simple ISO 8601 parsing — just need the span
    let parse_epoch_ms = |ts: &str| -> Option<u64> {
        // Try to parse "2026-02-15T21:00:00Z" or similar
        let ts = ts.trim();
        if ts.len() < 19 {
            return None;
        }
        let year: u64 = ts[0..4].parse().ok()?;
        let month: u64 = ts[5..7].parse().ok()?;
        let day: u64 = ts[8..10].parse().ok()?;
        let hour: u64 = ts[11..13].parse().ok()?;
        let min: u64 = ts[14..16].parse().ok()?;
        let sec: u64 = ts[17..19].parse().ok()?;
        // Rough epoch calculation (good enough for span estimation)
        Some(
            ((year - 2020) * 365 * 24 * 3600
                + month * 30 * 24 * 3600
                + day * 24 * 3600
                + hour * 3600
                + min * 60
                + sec)
                * 1000,
        )
    };

    let first_ms = parse_epoch_ms(first)?;
    let last_ms = parse_epoch_ms(last)?;
    let span_ms = last_ms.saturating_sub(first_ms);

    if span_ms == 0 {
        return None;
    }

    let span_hours = span_ms as f64 / (3600.0 * 1000.0);
    let hours_per_month = 730.0; // Average month

    let scale = hours_per_month / span_hours;
    let projected_invocations = (total_invocations as f64 * scale) as u64;
    let projected_gb_seconds = total_gb_seconds * scale;

    let projected_request_cost = projected_invocations as f64 * COST_PER_REQUEST;
    let projected_compute_cost = projected_gb_seconds * cost_per_gb_second;
    let projected_cost_before_free_tier = projected_request_cost + projected_compute_cost;

    // Free tier savings
    let free_request_savings =
        (projected_invocations.min(FREE_TIER_REQUESTS) as f64) * COST_PER_REQUEST;
    let free_compute_savings = projected_gb_seconds.min(FREE_TIER_GB_SECONDS) * cost_per_gb_second;
    let free_tier_savings = free_request_savings + free_compute_savings;

    let projected_cost_after_free_tier =
        (projected_cost_before_free_tier - free_tier_savings).max(0.0);

    Some(MonthlyProjection {
        projected_invocations,
        projected_gb_seconds,
        projected_cost_before_free_tier,
        free_tier_savings,
        projected_cost_after_free_tier,
        observation_hours: span_hours,
    })
}

/// Format a cost report for terminal display
pub fn format_report(report: &CostReport) -> String {
    let mut out = String::new();

    let arch_label = match report.architecture {
        Architecture::X86_64 => "x86_64",
        Architecture::Arm64 => "ARM64 (Graviton)",
    };

    out.push_str(&format!("\n💰 Lambda Cost Estimation ({})\n", arch_label));
    out.push_str(&"─".repeat(60));
    out.push('\n');

    if report.functions.is_empty() {
        out.push_str("\n  No invocations recorded yet.\n");
        out.push_str("  Run `lambdaform start` and make some requests first.\n\n");
        return out;
    }

    // Per-function breakdown
    out.push_str("\n  Function Breakdown:\n\n");

    for fc in &report.functions {
        out.push_str(&format!("  ⚡ {} ({}MB)\n", fc.function_name, fc.memory_mb));
        out.push_str(&format!(
            "     Invocations: {}  |  GB-seconds: {:.4}\n",
            fc.invocations, fc.gb_seconds
        ));
        out.push_str(&format!(
            "     Duration — avg: {:.1}ms  |  p95: {}ms  |  max: {}ms\n",
            fc.avg_duration_ms, fc.p95_duration_ms, fc.max_duration_ms
        ));
        out.push_str(&format!(
            "     Cost — requests: ${:.6}  |  compute: ${:.6}  |  total: ${:.6}\n\n",
            fc.request_cost, fc.compute_cost, fc.total_cost
        ));
    }

    // Totals
    out.push_str(&"─".repeat(60));
    out.push('\n');
    out.push_str(&format!(
        "  Total: {} invocations, {:.4} GB-seconds\n",
        report.total_invocations, report.total_gb_seconds
    ));
    out.push_str(&format!(
        "  Observed cost: ${:.6} (requests: ${:.6} + compute: ${:.6})\n",
        report.total_cost, report.total_request_cost, report.total_compute_cost
    ));

    // Monthly projection
    if let Some(ref proj) = report.monthly_projection {
        out.push('\n');
        out.push_str(&format!(
            "  📊 Monthly Projection (based on {:.1}h of data):\n",
            proj.observation_hours
        ));
        out.push_str(&format!(
            "     Projected invocations: ~{}\n",
            format_number(proj.projected_invocations)
        ));
        out.push_str(&format!(
            "     Projected GB-seconds:  ~{:.1}\n",
            proj.projected_gb_seconds
        ));
        out.push_str(&format!(
            "     Estimated monthly cost: ${:.4}\n",
            proj.projected_cost_before_free_tier
        ));
        if proj.free_tier_savings > 0.0 {
            out.push_str(&format!(
                "     Free tier savings:      -${:.4}\n",
                proj.free_tier_savings
            ));
            out.push_str(&format!(
                "     After free tier:         ${:.4}\n",
                proj.projected_cost_after_free_tier
            ));
        }
    }

    out.push('\n');
    out.push_str("  💡 Pricing: us-east-1, Jan 2025. Actual costs may vary.\n");
    out.push_str("     Free tier: 1M requests + 400K GB-seconds/month.\n\n");

    out
}

/// Format a cost report as JSON
pub fn format_report_json(report: &CostReport) -> serde_json::Value {
    let functions: Vec<serde_json::Value> = report
        .functions
        .iter()
        .map(|fc| {
            serde_json::json!({
                "function_name": fc.function_name,
                "resource_name": fc.resource_name,
                "memory_mb": fc.memory_mb,
                "invocations": fc.invocations,
                "total_duration_ms": fc.total_duration_ms,
                "avg_duration_ms": fc.avg_duration_ms,
                "p95_duration_ms": fc.p95_duration_ms,
                "max_duration_ms": fc.max_duration_ms,
                "gb_seconds": fc.gb_seconds,
                "request_cost_usd": fc.request_cost,
                "compute_cost_usd": fc.compute_cost,
                "total_cost_usd": fc.total_cost,
            })
        })
        .collect();

    let mut result = serde_json::json!({
        "architecture": match report.architecture {
            Architecture::X86_64 => "x86_64",
            Architecture::Arm64 => "arm64",
        },
        "total_invocations": report.total_invocations,
        "total_gb_seconds": report.total_gb_seconds,
        "total_request_cost_usd": report.total_request_cost,
        "total_compute_cost_usd": report.total_compute_cost,
        "total_cost_usd": report.total_cost,
        "functions": functions,
    });

    if let Some(ref proj) = report.monthly_projection {
        result["monthly_projection"] = serde_json::json!({
            "observation_hours": proj.observation_hours,
            "projected_invocations": proj.projected_invocations,
            "projected_gb_seconds": proj.projected_gb_seconds,
            "projected_cost_before_free_tier_usd": proj.projected_cost_before_free_tier,
            "free_tier_savings_usd": proj.free_tier_savings,
            "projected_cost_after_free_tier_usd": proj.projected_cost_after_free_tier,
        });
    }

    result
}

/// Format a large number with commas
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LambdaConfig, Runtime};
    use std::collections::HashMap;

    fn make_entry(function: &str, duration_ms: u64, timestamp: &str) -> HistoryEntry {
        HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: timestamp.to_string(),
            method: "GET".to_string(),
            path: "/test".to_string(),
            query: None,
            headers: None,
            body: None,
            function: function.to_string(),
            status: 200,
            response_body: None,
            duration_ms,
            port: 3000,
        }
    }

    fn make_lambda(name: &str, memory_mb: u32) -> LambdaConfig {
        LambdaConfig {
            resource_name: name.to_string(),
            function_name: name.to_string(),
            handler: "index.handler".to_string(),
            runtime: Runtime::Nodejs20,
            source_path: None,
            filename_ref: None,
            environment: HashMap::new(),
            timeout: 30,
            memory_size: memory_mb,
            layers: Vec::new(),
            architecture: crate::config::Architecture::default(),
        }
    }

    #[test]
    fn test_empty_history() {
        let report = estimate_costs(&[], &[], Architecture::X86_64);
        assert_eq!(report.total_invocations, 0);
        assert_eq!(report.total_cost, 0.0);
    }

    #[test]
    fn test_single_function_cost() {
        let entries = vec![
            make_entry("api_handler", 100, "2026-02-16T10:00:00Z"),
            make_entry("api_handler", 200, "2026-02-16T10:00:01Z"),
            make_entry("api_handler", 150, "2026-02-16T10:00:02Z"),
        ];
        let functions = vec![make_lambda("api_handler", 256)];
        let report = estimate_costs(&entries, &functions, Architecture::X86_64);

        assert_eq!(report.total_invocations, 3);
        assert_eq!(report.functions.len(), 1);

        let fc = &report.functions[0];
        assert_eq!(fc.invocations, 3);
        assert_eq!(fc.memory_mb, 256);
        assert_eq!(fc.total_duration_ms, 450);
        assert!((fc.avg_duration_ms - 150.0).abs() < 0.1);
        assert_eq!(fc.p95_duration_ms, 200);

        // GB-seconds = (256/1024) * (450/1000) = 0.25 * 0.45 = 0.1125
        assert!((fc.gb_seconds - 0.1125).abs() < 0.0001);

        // Request cost = 3 * $0.0000002 = $0.0000006
        assert!((fc.request_cost - 0.0000006).abs() < 1e-10);

        assert!(fc.total_cost > 0.0);
    }

    #[test]
    fn test_multiple_functions() {
        let entries = vec![
            make_entry("func_a", 50, "2026-02-16T10:00:00Z"),
            make_entry("func_b", 300, "2026-02-16T10:00:01Z"),
            make_entry("func_a", 75, "2026-02-16T10:00:02Z"),
        ];
        let functions = vec![make_lambda("func_a", 128), make_lambda("func_b", 512)];
        let report = estimate_costs(&entries, &functions, Architecture::X86_64);

        assert_eq!(report.total_invocations, 3);
        assert_eq!(report.functions.len(), 2);
    }

    #[test]
    fn test_arm_pricing() {
        let entries = vec![make_entry("handler", 1000, "2026-02-16T10:00:00Z")];
        let functions = vec![make_lambda("handler", 1024)];

        let x86_report = estimate_costs(&entries, &functions, Architecture::X86_64);
        let arm_report = estimate_costs(&entries, &functions, Architecture::Arm64);

        // ARM should be cheaper
        assert!(arm_report.total_cost < x86_report.total_cost);
    }

    #[test]
    fn test_monthly_projection() {
        // 10 requests over 1 hour
        let entries: Vec<HistoryEntry> = (0..10)
            .map(|i| make_entry("handler", 100, &format!("2026-02-16T10:{:02}:00Z", i * 6)))
            .collect();
        let functions = vec![make_lambda("handler", 128)];
        let report = estimate_costs(&entries, &functions, Architecture::X86_64);

        assert!(report.monthly_projection.is_some());
        let proj = report.monthly_projection.unwrap();
        assert!(proj.projected_invocations > 10);
        assert!(proj.observation_hours > 0.0);
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1_000_000), "1,000,000");
    }

    #[test]
    fn test_json_output() {
        let entries = vec![make_entry("handler", 100, "2026-02-16T10:00:00Z")];
        let functions = vec![make_lambda("handler", 256)];
        let report = estimate_costs(&entries, &functions, Architecture::X86_64);
        let json = format_report_json(&report);

        assert_eq!(json["total_invocations"], 1);
        assert_eq!(json["architecture"], "x86_64");
        assert!(json["functions"].is_array());
    }
}
