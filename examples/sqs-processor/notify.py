"""Notification Sender — SQS-triggered Lambda (Python)
Processes notification messages from the notifications queue.
Demonstrates Python SQS handler with JSON parsing and logging.
"""

import json
import os
import logging

logger = logging.getLogger()
logger.setLevel(logging.INFO)


def handler(event, context):
    environment = os.environ.get("ENVIRONMENT", "unknown")
    logger.info(f"Notification sender running in {environment}")
    logger.info(f"Processing {len(event['Records'])} notification(s)")

    sent = []
    errors = []

    for record in event["Records"]:
        message_id = record["messageId"]
        logger.info(f"Processing notification {message_id}")

        try:
            notification = json.loads(record["body"])

            # Validate notification
            if "type" not in notification or "recipient" not in notification:
                raise ValueError("Missing 'type' or 'recipient' in notification")

            notif_type = notification["type"]
            recipient = notification["recipient"]
            message = notification.get("message", "No message")

            logger.info(f"  Type: {notif_type}")
            logger.info(f"  Recipient: {recipient}")
            logger.info(f"  Message: {message}")

            # Log SQS metadata
            logger.info(f"  Source ARN: {record['eventSourceARN']}")
            logger.info(f"  MD5: {record['md5OfBody']}")

            sent.append({
                "messageId": message_id,
                "type": notif_type,
                "recipient": recipient,
                "status": "sent",
            })

        except Exception as e:
            logger.error(f"Failed to process notification {message_id}: {e}")
            errors.append({"messageId": message_id, "error": str(e)})

    result = {
        "sent": len(sent),
        "errors": len(errors),
        "details": sent,
    }

    logger.info(f"Complete: {len(sent)} sent, {len(errors)} errors")
    return result
