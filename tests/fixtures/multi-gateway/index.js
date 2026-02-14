exports.handler = async (event) => {
  const path = event.path || '/';
  return {
    statusCode: 200,
    body: JSON.stringify({ message: `Hello from ${path}`, gateway: event.requestContext.apiId }),
  };
};
