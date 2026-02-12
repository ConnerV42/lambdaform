/**
 * Echo Lambda handler - returns whatever was sent in the body
 */
exports.handler = async (event, context) => {
  console.log('Echo received:', event.body);

  let body;
  try {
    body = event.body ? JSON.parse(event.body) : {};
  } catch (e) {
    body = { raw: event.body };
  }

  return {
    statusCode: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      echo: body,
      method: event.httpMethod,
      path: event.path,
      timestamp: new Date().toISOString(),
    }),
  };
};
