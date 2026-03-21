exports.handler = async (event) => {
  const pageSize = parseInt(process.env.PAGE_SIZE || '25');
  const page = parseInt(event.queryStringParameters?.page || '1');
  
  // Simulated items
  const items = Array.from({ length: pageSize }, (_, i) => ({
    id: `item-${(page - 1) * pageSize + i + 1}`,
    name: `Item ${(page - 1) * pageSize + i + 1}`,
    createdAt: new Date().toISOString(),
  }));

  return {
    statusCode: 200,
    headers: {
      'Content-Type': 'application/json',
      'Access-Control-Allow-Origin': process.env.CORS_ORIGINS?.split(',')[0] || '*',
    },
    body: JSON.stringify({
      items,
      page,
      pageSize,
      environment: process.env.ENVIRONMENT,
      region: process.env.REGION,
      logLevel: process.env.LOG_LEVEL,
    }),
  };
};
