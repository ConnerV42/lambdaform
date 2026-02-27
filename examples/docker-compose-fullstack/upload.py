import json
import os
import uuid
import base64
import boto3
from datetime import datetime

dynamodb = boto3.resource('dynamodb', endpoint_url=os.environ.get('DYNAMODB_URL'),
                          region_name='us-east-1', aws_access_key_id='local', aws_secret_access_key='local')
s3 = boto3.client('s3', endpoint_url=os.environ.get('S3_URL'),
                   region_name='us-east-1', aws_access_key_id='local', aws_secret_access_key='local')

TABLE = os.environ.get('TABLE_NAME', 'uploads')
BUCKET = os.environ.get('BUCKET_NAME', 'uploads-bucket')


def handler(event, context):
    try:
        body = json.loads(event.get('body', '{}'))
        file_content = body.get('content', '')
        filename = body.get('filename', 'unnamed.txt')

        upload_id = str(uuid.uuid4())
        s3_key = f"uploads/{upload_id}/{filename}"

        # Upload to S3
        s3.put_object(
            Bucket=BUCKET,
            Key=s3_key,
            Body=base64.b64decode(file_content) if body.get('base64') else file_content.encode(),
            ContentType=body.get('content_type', 'application/octet-stream'),
        )

        # Record in DynamoDB
        table = dynamodb.Table(TABLE)
        table.put_item(Item={
            'id': upload_id,
            'filename': filename,
            's3_key': s3_key,
            'size': len(file_content),
            'created_at': datetime.utcnow().isoformat(),
        })

        return {
            'statusCode': 201,
            'headers': {'Content-Type': 'application/json'},
            'body': json.dumps({'id': upload_id, 'filename': filename, 's3_key': s3_key}),
        }
    except Exception as e:
        return {
            'statusCode': 500,
            'headers': {'Content-Type': 'application/json'},
            'body': json.dumps({'error': str(e)}),
        }
