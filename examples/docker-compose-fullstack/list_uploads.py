import json
import os
import boto3

dynamodb = boto3.resource('dynamodb', endpoint_url=os.environ.get('DYNAMODB_URL'),
                          region_name='us-east-1', aws_access_key_id='local', aws_secret_access_key='local')

TABLE = os.environ.get('TABLE_NAME', 'uploads')


def handler(event, context):
    try:
        table = dynamodb.Table(TABLE)
        upload_id = (event.get('pathParameters') or {}).get('id')

        if upload_id:
            result = table.get_item(Key={'id': upload_id})
            item = result.get('Item')
            if not item:
                return respond(404, {'error': 'Not found'})
            return respond(200, item)

        result = table.scan()
        items = result.get('Items', [])
        return respond(200, items)
    except Exception as e:
        return respond(500, {'error': str(e)})


def respond(status, body):
    return {
        'statusCode': status,
        'headers': {'Content-Type': 'application/json'},
        'body': json.dumps(body, default=str),
    }
