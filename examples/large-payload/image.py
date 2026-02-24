"""Image processor — accepts image upload, returns metadata about it."""
import json
import base64


def handler(event, context):
    body = event.get("body", "")
    is_base64 = event.get("isBase64Encoded", False)
    
    if is_base64:
        data = base64.b64decode(body)
    else:
        data = body.encode("utf-8") if isinstance(body, str) else body
    
    # Detect image type from magic bytes
    image_type = "unknown"
    if data[:8] == b'\x89PNG\r\n\x1a\n':
        image_type = "png"
    elif data[:2] == b'\xff\xd8':
        image_type = "jpeg"
    elif data[:4] == b'GIF8':
        image_type = "gif"
    elif data[:4] == b'RIFF' and data[8:12] == b'WEBP':
        image_type = "webp"
    
    return {
        "statusCode": 200,
        "headers": {"Content-Type": "application/json"},
        "body": json.dumps({
            "imageType": image_type,
            "sizeBytes": len(data),
            "isBase64Encoded": is_base64,
            "firstBytes": data[:16].hex() if len(data) >= 16 else data.hex(),
        }),
    }
