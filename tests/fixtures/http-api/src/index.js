/**
 * Simple hello world Lambda handler for testing Lambdaform
 */
exports.handler = async (event, context) => {
  console.log('Event:', JSON.stringify(event, null, 2));
  console.log('Context:', JSON.stringify(context, null, 2));

  const greeting = process.env.GREETING || 'Hello, World!';
  const env = process.env.ENV || 'unknown';

  const name = event.queryStringParameters?.name || 'stranger';

  return {
    statusCode: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      message: `${greeting} Welcome, ${name}!`,
      environment: env,
      timestamp: new Date().toISOString(),
      requestId: context.awsRequestId,
    }),
  };
};
