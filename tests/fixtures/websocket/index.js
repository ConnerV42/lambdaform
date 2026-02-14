// WebSocket Lambda handlers for testing

exports.connect = async (event, context) => {
  console.error(`[connect] connectionId: ${event.requestContext.connectionId}`);
  return { statusCode: 200, body: 'Connected.' };
};

exports.disconnect = async (event, context) => {
  console.error(`[disconnect] connectionId: ${event.requestContext.connectionId}`);
  return { statusCode: 200, body: 'Disconnected.' };
};

exports.default = async (event, context) => {
  console.error(`[default] connectionId: ${event.requestContext.connectionId}, body: ${event.body}`);
  return {
    statusCode: 200,
    body: JSON.stringify({ message: 'Default route', echo: event.body }),
  };
};

exports.sendmessage = async (event, context) => {
  const body = JSON.parse(event.body || '{}');
  console.error(`[sendmessage] connectionId: ${event.requestContext.connectionId}, data: ${body.data}`);
  return {
    statusCode: 200,
    body: JSON.stringify({ message: 'Message sent', data: body.data }),
  };
};
