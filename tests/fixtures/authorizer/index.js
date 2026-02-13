// Simple handler for both protected and public endpoints
exports.handler = async (event, context) => {
  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      message: `Hello from ${event.path}`,
      method: event.httpMethod,
    }),
  };
};
