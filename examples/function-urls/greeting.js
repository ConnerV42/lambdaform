// Greeting function — served via Lambda Function URL
// Each Function URL gets its own port in Lambdaform

exports.handler = async (event) => {
  const name = event.queryStringParameters?.name || "World";
  const appName = process.env.APP_NAME || "Lambda";

  return {
    statusCode: 200,
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      message: `Hello, ${name}! Welcome to ${appName}.`,
      timestamp: new Date().toISOString(),
      functionUrl: true,
      method: event.requestContext?.http?.method || "GET",
    }),
  };
};
