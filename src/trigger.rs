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
        let md5 = md5_hash(msg);

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

    let records: Vec<Value> = messages
        .iter()
        .map(|msg| {
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
        })
        .collect();

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
            let queue = config
                .sqs_queues
                .iter()
                .find(|q| q.resource_name == source_name || q.name == source_name)
                .with_context(|| {
                    format!(
                        "SQS queue '{}' not found. Available: {}",
                        source_name,
                        config
                            .sqs_queues
                            .iter()
                            .map(|q| q.resource_name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            (
                EventSourceType::Sqs,
                queue.resource_name.clone(),
                Some(queue.clone()),
                None,
            )
        }
        "sns" => {
            let topic = config
                .sns_topics
                .iter()
                .find(|t| t.resource_name == source_name || t.name == source_name)
                .with_context(|| {
                    format!(
                        "SNS topic '{}' not found. Available: {}",
                        source_name,
                        config
                            .sns_topics
                            .iter()
                            .map(|t| t.resource_name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            (
                EventSourceType::Sqs,
                topic.resource_name.clone(),
                None,
                Some(topic.clone()),
            ) // source_type doesn't matter for lookup
        }
        _ => anyhow::bail!(
            "Unsupported trigger type '{}'. Use 'sqs' or 'sns'.",
            source_type
        ),
    };

    // For SQS: find event source mapping
    // For SNS: find via sns_topic_subscription or event_source_mapping
    let function_resource = if source_type == "sqs" {
        let esm = config
            .event_source_mappings
            .iter()
            .find(|e| {
                e.source_type == EventSourceType::Sqs
                    && e.source_resource == resolved_resource
                    && e.enabled
            })
            .with_context(|| {
                format!(
                    "No event source mapping found for SQS queue '{}'. \
                 Make sure you have an aws_lambda_event_source_mapping resource.",
                    source_name
                )
            })?;
        esm.function_resource.clone()
    } else {
        // For SNS, look for event source mapping (some people use it) or fall back
        // SNS → Lambda is typically via aws_sns_topic_subscription, not event_source_mapping
        // But we'll check ESM first, then look for any function that references this topic
        config
            .event_source_mappings
            .iter()
            .find(|e| e.source_resource == resolved_resource && e.enabled)
            .map(|e| e.function_resource.clone())
            .unwrap_or_else(|| {
                // Fallback: use the first function (user can specify via --function flag)
                config
                    .functions
                    .first()
                    .map(|f| f.resource_name.clone())
                    .unwrap_or_default()
            })
    };

    // Find the Lambda function
    let lambda = config
        .functions
        .iter()
        .find(|f| f.resource_name == function_resource || f.function_name == function_resource)
        .with_context(|| format!("Lambda function '{}' not found", function_resource))?;

    // Build the event
    let event_payload = match source_type {
        "sqs" => {
            let q = queue_name.expect("queue_name required for sqs source type");
            build_sqs_event(&q.name, &messages, q.fifo_queue)
        }
        "sns" => {
            let t = topic_name.expect("topic_name required for sns source type");
            build_sns_event(&t.name, &messages)
        }
        _ => unreachable!(),
    };

    println!(
        "⚡ Triggering {} → {} with {} message(s)",
        source_name,
        lambda.function_name,
        messages.len()
    );

    let fn_source_dir = lambda.resolve_source_dir_with_archives(source_dir, &config.archive_files);
    let executor = FunctionExecutor::new(lambda.clone(), fn_source_dir);
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
    // ISO 8601 format — compute full date from epoch seconds
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Days since epoch
    let days = now / 86400;
    let time_of_day = now % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Civil date from days since 1970-01-01 (algorithm from Howard Hinnant)
    let z = days as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000Z",
        y, m, d, hours, minutes, seconds
    )
}

/// Compute MD5 hash of a string (matches AWS md5OfBody format)
fn md5_hash(s: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
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
        assert!(records[0]["Sns"]["TopicArn"]
            .as_str()
            .unwrap()
            .contains("my-topic"));
    }

    #[test]
    fn test_build_sns_event_batch() {
        let msgs = vec!["x".to_string(), "y".to_string()];
        let event = build_sns_event("t", &msgs);
        assert_eq!(event["Records"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_md5_hash_known_value() {
        // MD5 of "hello" is well-known
        assert_eq!(md5_hash("hello"), "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_md5_hash_empty() {
        assert_eq!(md5_hash(""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_timestamp_ms_is_numeric() {
        let ts = timestamp_ms();
        assert!(ts.parse::<u128>().is_ok(), "timestamp_ms should be numeric");
        assert!(ts.len() >= 13, "should be millisecond precision");
    }

    #[test]
    fn test_chrono_timestamp_format() {
        let ts = chrono_timestamp();
        // Should match YYYY-MM-DDTHH:MM:SS.000Z
        assert!(ts.ends_with(".000Z"), "should end with .000Z: {}", ts);
        assert_eq!(
            ts.len(),
            24,
            "ISO 8601 with millis should be 24 chars: {}",
            ts
        );
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
    }

    #[test]
    fn test_sqs_event_has_md5_of_body() {
        let event = build_sqs_event("q", &["test body".to_string()], false);
        let md5 = event["Records"][0]["md5OfBody"].as_str().unwrap();
        assert_eq!(md5, md5_hash("test body"));
    }

    #[test]
    fn test_sqs_event_arn_format() {
        let event = build_sqs_event("my-queue", &["x".to_string()], false);
        let arn = event["Records"][0]["eventSourceARN"].as_str().unwrap();
        assert!(arn.starts_with("arn:aws:sqs:"));
        assert!(arn.ends_with("my-queue"));
    }

    #[test]
    fn test_sns_event_structure() {
        let event = build_sns_event("alerts", &["alert!".to_string()]);
        let record = &event["Records"][0];
        assert_eq!(record["EventVersion"], "1.0");
        assert!(record["Sns"]["MessageId"].is_string());
        assert!(record["Sns"]["Timestamp"].is_string());
        assert_eq!(record["Sns"]["Type"], "Notification");
    }

    #[test]
    fn test_sqs_event_receipt_handle_nonempty() {
        let event = build_sqs_event("q", &["msg".to_string()], false);
        let handle = event["Records"][0]["receiptHandle"].as_str().unwrap();
        assert!(!handle.is_empty());
        assert!(handle.starts_with("AQEBwJnK"));
    }

    #[test]
    fn test_sqs_event_attributes_present() {
        let event = build_sqs_event("q", &["msg".to_string()], false);
        let attrs = &event["Records"][0]["attributes"];
        assert!(attrs["ApproximateReceiveCount"].is_string());
        assert!(attrs["SentTimestamp"].is_string());
        assert!(attrs["SenderId"].is_string());
        assert!(attrs["ApproximateFirstReceiveTimestamp"].is_string());
    }

    #[test]
    fn test_sqs_fifo_has_sequence_number() {
        let event = build_sqs_event("q.fifo", &["msg".to_string()], true);
        let attrs = &event["Records"][0]["attributes"];
        assert!(attrs["SequenceNumber"].is_string());
        assert!(attrs["MessageDeduplicationId"].is_string());
        assert_eq!(attrs["MessageGroupId"], "lambdaform");
    }

    #[test]
    fn test_sqs_event_unique_message_ids() {
        let msgs = vec!["a".to_string(), "b".to_string()];
        let event = build_sqs_event("q", &msgs, false);
        let records = event["Records"].as_array().unwrap();
        let id0 = records[0]["messageId"].as_str().unwrap();
        let id1 = records[1]["messageId"].as_str().unwrap();
        assert_ne!(id0, id1, "each message should have a unique ID");
    }

    #[test]
    fn test_sns_event_subscription_arn_format() {
        let event = build_sns_event("my-topic", &["msg".to_string()]);
        let sub_arn = event["Records"][0]["EventSubscriptionArn"]
            .as_str()
            .unwrap();
        assert!(sub_arn.contains("my-topic"));
        assert!(sub_arn.ends_with(":lambdaform-sub"));
    }

    #[test]
    fn test_sns_event_null_subject() {
        let event = build_sns_event("t", &["msg".to_string()]);
        assert!(event["Records"][0]["Sns"]["Subject"].is_null());
    }

    #[test]
    fn test_chrono_timestamp_civil_date_valid() {
        let ts = chrono_timestamp();
        // Parse the date parts and verify they're reasonable
        let year: i32 = ts[0..4].parse().unwrap();
        let month: u32 = ts[5..7].parse().unwrap();
        let day: u32 = ts[8..10].parse().unwrap();
        assert!(year >= 2026 && year <= 2100, "year out of range: {}", year);
        assert!((1..=12).contains(&month), "month out of range: {}", month);
        assert!((1..=31).contains(&day), "day out of range: {}", day);
    }

    #[test]
    fn test_chrono_timestamp_time_valid() {
        let ts = chrono_timestamp();
        let hours: u32 = ts[11..13].parse().unwrap();
        let minutes: u32 = ts[14..16].parse().unwrap();
        let seconds: u32 = ts[17..19].parse().unwrap();
        assert!(hours < 24, "hours out of range: {}", hours);
        assert!(minutes < 60, "minutes out of range: {}", minutes);
        assert!(seconds < 60, "seconds out of range: {}", seconds);
    }

    #[test]
    fn test_sqs_empty_messages() {
        let event = build_sqs_event("q", &[], false);
        let records = event["Records"].as_array().unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn test_sns_empty_messages() {
        let event = build_sns_event("t", &[]);
        let records = event["Records"].as_array().unwrap();
        assert!(records.is_empty());
    }
}
