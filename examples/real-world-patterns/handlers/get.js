exports.handler = async (event) => {
  const id = event.pathParameters?.id;
  
  if (!id) {
    return {
      statusCode: 400,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Missing id parameter' }),
    };
  }

  // Simulated item lookup
  return {
    statusCode: 200,
    headers: {
      'Content-Type': 'application/json',
      'Access-Control-Allow-Origin': process.env.CORS_ORIGINS?.split(',')[0] || '*',
    },
    body: JSON.stringify({
      id,
      name: `Item ${id}`,
      description: 'A detailed item retrieved by ID',
      table: process.env.TABLE_NAME,
      environment: process.env.ENVIRONMENT,
      purpose: process.env.FUNCTION_PURPOSE,
    }),
  };
};
