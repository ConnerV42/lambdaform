import json
from datetime import datetime, timedelta

def handler(event, context):
    print(f"Shipping order: {json.dumps(event)}")
    
    order_id = event.get('orderId', 'unknown')
    
    return {
        'orderId': order_id,
        'trackingNumber': f"TRACK-{order_id}-{int(datetime.now().timestamp())}",
        'estimatedDelivery': (datetime.now() + timedelta(days=3)).isoformat(),
        'carrier': 'FastShip',
        'shippedAt': datetime.now().isoformat()
    }
