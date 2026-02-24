import json
import os
from datetime import datetime

def handler(event, context):
    print(f"Processing payment: {json.dumps(event)}")
    
    gateway_url = os.environ.get('PAYMENT_GATEWAY_URL', 'https://mock-payment.example.com')
    amount = event.get('amount', 0)
    
    # Simulate payment processing
    if amount > 10000:
        raise Exception('PaymentFailed: Amount exceeds limit')
    
    return {
        **event,
        'paymentId': f"pay_{event.get('orderId', 'unknown')}_{int(datetime.now().timestamp())}",
        'paymentStatus': 'completed',
        'gateway': gateway_url,
        'processedAt': datetime.now().isoformat()
    }
