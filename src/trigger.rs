//! SQS/SNS trigger simulation
//!
//! Sends test messages through event source mappings to invoke Lambda functions
//! with properly-formatted SQS or SNS event payloads.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

use crate::config::*;
use crate::runtime::FunctionExecutor;

/// Build an SQS event payload for a batch of messages
pub fn build_sqs_event(queue_name: &str, messages: &[String], fifo: bool) -> Value {
    let region = "us-east-1";
    let account = "123456789012";
    let queue_arn = format!("arn:aws:sqs:{}:{}:{}", region, account, queue_name);
    
    let records: Vec<Value> = messages.iter().map(|msg| {
        let message_id = Uuid::new_v4().to_string();
        let receipt_handle = format!("AQEBwJnK{}", Uuid::new_v4().to_string().replace('-', ""));
        let md5 = format!("{:x}", md5_hash(msg));
        
        let mut record = json!({
            "messageId": message_id,
            "receiptHandle": receipt_handle,
            "body": msg,
            "attributes": {
                "ApproximateReceiveCount": "1",
                "SentTimestamp": timestamp_ms(),
                "SenderId": format!("AROA{}:lambdaform", Uuid::new_v4().to_string().replace('-', "")[..12].to_uppercase()),
                "ApproximateFirstReceiveTimestamp": timestamp_ms()
            },
            "messageAttributes": {},
            "md5OfBody": md5,
            "eventSource": "aws:sqs",
            "eventSourceARN": queue_arn,
            "awsRegion": region
        });
        
        if fifo {
            record["attributes"]["MessageGroupId"] = json!("lambdaform");
            record["attributes"]["MessageDeduplicationId"] = json!(message_id);
            record["attributes"]["SequenceNumber"] = json!("18849496460467696128");
        }
        
        record
    }).collect();
    
    json!({ "Records": records })
}

/// Build an SNS event payload wrapping messages for Lambda invocation via SNS→Lambda subscription
pub fn build_sns_event(topic_name: &str, messages: &[String]) -> Value {
    let region = "us-east-1";
    let account = "123456789012";
    let topic_arn = format!("arn:aws:sns:{}:{}:{}", region, account, topic_name);
    
    let records: Vec<Value> = messages.iter().map(|msg| {
        let message_id = Uuid::new_v4().to_string();
        let ts = chrono_timestamp();
        
        json!({
            "EventVersion": "1.0",
            "EventSubscriptionArn": format!("{}:lambdaform-sub", topic_arn),
            "EventSource": "aws:sns",
            "Sns": {
                "SignatureVersion": "1",
                "Timestamp": ts,
                "Signature": "EXAMPLE",
                "SigningCertUrl": "EXAMPLE",
                "MessageId": message_id,
                "Message": msg,
                "MessageAttributes": {},
                "Type": "Notification",
                "UnsubscribeUrl": "EXAMPLE",
                "TopicArn": topic_arn,
                "Subject": serde_json::Value::Null
            }
        })
    }).collect();
    
    json!({ "Records": records })
}

/// Execute a trigger: find the mapped Lambda and invoke it with the formatted event
pub async fn execute_trigger(
    config: &LambdaformConfig,
    source_type: &str,
    source_name: &str,
    messages: Vec<String>,
    source_dir: &Path,
) -> Result<()> {
    // Find the source resource
    let (_resolved_type, resolved_resource, queue_name, topic_name) = match source_type {
        "sqs" => {
            let queue = config.sqs_queues.iter()
                .find(|q| q.resource_name == source_name || q.name == source_name)
                .with_context(|| format!("SQS queue '{}' not found. Available: {}",
                    source_name,
                    config.sqs_queues.iter().map(|q| q.resource_name.as_str()).collect::<Vec<_>>().join(", ")
                ))?;
            (EventSourceType::Sqs, queue.resource_name.clone(), Some(queue.clone()), None)
        }
        "sns" => {
            let topic = config.sns_topics.iter()
                .find(|t| t.resource_name == source_name || t.name == source_name)
                .with_context(|| format!("SNS topic '{}' not found. Available: {}",
                    source_name,
                    config.sns_topics.iter().map(|t| t.resource_name.as_str()).collect::<Vec<_>>().join(", ")
                ))?;
            (EventSourceType::Sqs, topic.resource_name.clone(), None, Some(topic.clone())) // source_type doesn't matter for lookup
        }
        _ => anyhow::bail!("Unsupported trigger type '{}'. Use 'sqs' or 'sns'.", source_type),
    };
    
    // For SQS: find event source mapping
    // For SNS: find via sns_topic_subscription or event_source_mapping
    let function_resource = if source_type == "sqs" {
        let esm = config.event_source_mappings.iter()
            .find(|e| e.source_type == EventSourceType::Sqs && e.source_resource == resolved_resource && e.enabled)
            .with_context(|| format!(
                "No event source mapping found for SQS queue '{}'. \
                 Make sure you have an aws_lambda_event_source_mapping resource.", source_name
            ))?;
        esm.function_resource.clone()
    } else {
        // For SNS, look for event source mapping (some people use it) or fall back
        // SNS → Lambda is typically via aws_sns_topic_subscription, not event_source_mapping
        // But we'll check ESM first, then look for any function that references this topic
        config.event_source_mappings.iter()
            .find(|e| e.source_resource == resolved_resource && e.enabled)
            .map(|e| e.function_resource.clone())
            .unwrap_or_else(|| {
                // Fallback: use the first function (user can specify via --function flag)
                config.functions.first().map(|f| f.resource_name.clone()).unwrap_or_default()
            })
    };
    
    // Find the Lambda function
    let lambda = config.functions.iter()
        .find(|f| f.resource_name == function_resource || f.function_name == function_resource)
        .with_context(|| format!("Lambda function '{}' not found", function_resource))?;
    
    // Build the event
    let event_payload = match source_type {
        "sqs" => {
            let q = queue_name.unwrap();
            build_sqs_event(&q.name, &messages, q.fifo_queue)
        }
        "sns" => {
            let t = topic_name.unwrap();
            build_sns_event(&t.name, &messages)
        }
        _ => unreachable!(),
    };
    
    println!("⚡ Triggering {} → {} with {} message(s)",
        source_name, lambda.function_name, messages.len());
    
    let executor = FunctionExecutor::new(lambda.clone(), source_dir.to_path_buf());
    match executor.invoke_raw_event(event_payload).await {
        Ok(result) => {
            println!("✅ Invocation successful");
            println!("{}", serde_json::to_string_pretty(&result)?);
            Ok(())
        }
        Err(e) => {
            println!("❌ Invocation failed: {}", e);
            Err(e)
        }
    }
}

fn timestamp_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn chrono_timestamp() -> String {
    // ISO 8601 format
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple UTC timestamp
    format!("2026-02-14T{:02}:{:02}:{:02}.000Z", (now / 3600) % 24, (now / 60) % 60, now % 60)
}

/// Simple string hash for md5OfBody (not cryptographic, just for simulation)
fn md5_hash(s: &str) -> u128 {
    let mut h: u128 = 0;
    for b in s.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u128);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_sqs_event_standard() {
        let event = build_sqs_event("my-queue", &["hello".to_string()], false);
        let records = event["Records"].as_array().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["body"], "hello");
        assert_eq!(records[0]["eventSource"], "aws:sqs");
        assert!(records[0]["attributes"]["MessageGroupId"].is_null());
    }

    #[test]
    fn test_build_sqs_event_fifo() {
        let event = build_sqs_event("my-queue.fifo", &["msg1".to_string()], true);
        let records = event["Records"].as_array().unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0]["attributes"]["MessageGroupId"].is_string());
    }

    #[test]
    fn test_build_sqs_event_batch() {
        let msgs = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let event = build_sqs_event("q", &msgs, false);
        assert_eq!(event["Records"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_build_sns_event() {
        let event = build_sns_event("my-topic", &["hello sns".to_string()]);
        let records = event["Records"].as_array().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["EventSource"], "aws:sns");
        assert_eq!(records[0]["Sns"]["Message"], "hello sns");
        assert!(records[0]["Sns"]["TopicArn"].as_str().unwrap().contains("my-topic"));
    }

    #[test]
    fn test_build_sns_event_batch() {
        let msgs = vec!["x".to_string(), "y".to_string()];
        let event = build_sns_event("t", &msgs);
        assert_eq!(event["Records"].as_array().unwrap().len(), 2);
    }
}
