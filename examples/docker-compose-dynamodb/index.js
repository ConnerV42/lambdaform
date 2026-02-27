const { DynamoDBClient, PutItemCommand, GetItemCommand, ScanCommand, DeleteItemCommand } = require('@aws-sdk/client-dynamodb');

const client = new DynamoDBClient({
  endpoint: process.env.AWS_ENDPOINT_URL || undefined,
  region: process.env.AWS_REGION || 'us-east-1',
  credentials: { accessKeyId: 'local', secretAccessKey: 'local' },
});

const TABLE = process.env.TABLE_NAME || 'items';

exports.handler = async (event) => {
  const method = event.requestContext?.http?.method || event.httpMethod;
  const path = event.requestContext?.http?.path || event.path;
  const id = event.pathParameters?.id;

  try {
    if (method === 'GET' && !id) {
      const result = await client.send(new ScanCommand({ TableName: TABLE }));
      const items = (result.Items || []).map(i => ({
        id: i.id.S, name: i.name?.S, createdAt: i.createdAt?.S,
      }));
      return respond(200, items);
    }

    if (method === 'GET' && id) {
      const result = await client.send(new GetItemCommand({
        TableName: TABLE, Key: { id: { S: id } },
      }));
      if (!result.Item) return respond(404, { error: 'Not found' });
      return respond(200, { id: result.Item.id.S, name: result.Item.name?.S });
    }

    if (method === 'POST') {
      const body = JSON.parse(event.body || '{}');
      const item = {
        id: { S: body.id || crypto.randomUUID() },
        name: { S: body.name || 'unnamed' },
        createdAt: { S: new Date().toISOString() },
      };
      await client.send(new PutItemCommand({ TableName: TABLE, Item: item }));
      return respond(201, { id: item.id.S, name: item.name.S });
    }

    if (method === 'DELETE' && id) {
      await client.send(new DeleteItemCommand({
        TableName: TABLE, Key: { id: { S: id } },
      }));
      return respond(200, { deleted: id });
    }

    return respond(405, { error: 'Method not allowed' });
  } catch (err) {
    console.error('Error:', err);
    return respond(500, { error: err.message });
  }
};

function respond(status, body) {
  return { statusCode: status, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
}
