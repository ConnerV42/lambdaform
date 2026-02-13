/**
 * User lookup handler — tests path parameters
 */
exports.handler = async (event, context) => {
  const userId = event.pathParameters?.id || 'unknown';

  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      userId,
      message: `User ${userId} found`,
    }),
  };
};
