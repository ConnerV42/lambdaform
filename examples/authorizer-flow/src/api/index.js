// Protected/Public API handler

exports.handler = async (event) => {
  console.log('API event:', JSON.stringify(event, null, 2));

  const path = event.path || '/';
  const method = event.httpMethod || 'GET';

  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      message: path === '/public' ? 'This is public!' : 'Welcome to the protected zone!',
      path,
      method,
      timestamp: new Date().toISOString(),
    }),
  };
};
