"""Order notification consumer.
Receives SNS messages from the orders topic and sends notifications.
"""
import json
import os
from datetime import datetime


def handler(event, context):
    print("Notifier handler invoked")
    
    notifications = []
    
    for record in event["Records"]:
        sns_message = record["Sns"]
        message = json.loads(sns_message["Message"])
        message_id = sns_message["MessageId"]
        
        print(f"Sending notification for order: {json.dumps(message)}")
        print(f"SNS MessageId: {message_id}")
        
        # Simulate sending notification
        notification = {
            "orderId": message.get("orderId", "unknown"),
            "type": "email",
            "service": os.environ.get("SERVICE", "notifier"),
            "sentAt": datetime.now().isoformat(),
            "recipient": message.get("customerEmail", "customer@example.com"),
            "subject": f"Order {message.get('orderId', 'N/A')} confirmed",
        }
        
        notifications.append(notification)
    
    return {
        "statusCode": 200,
        "body": json.dumps({
            "message": f"Sent {len(notifications)} notifications",
            "notifications": notifications,
        }),
    }
