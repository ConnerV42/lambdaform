exports.handler = async (event) => {
  let body;
  try {
    body = JSON.parse(event.body || '{}');
  } catch (e) {
    return {
      statusCode: 400,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Invalid JSON body' }),
    };
  }

  if (!body.name) {
    return {
      statusCode: 400,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Missing required field: name' }),
    };
  }

  const item = {
    id: `item-${Date.now()}`,
    name: body.name,
    description: body.description || '',
    createdAt: new Date().toISOString(),
    table: process.env.TABLE_NAME,
    environment: process.env.ENVIRONMENT,
  };

  return {
    statusCode: 201,
    headers: {
      'Content-Type': 'application/json',
      'Access-Control-Allow-Origin': process.env.CORS_ORIGINS?.split(',')[0] || '*',
    },
    body: JSON.stringify(item),
  };
};
