// $default handler — catches unmatched routes
exports.handler = async (event) => {
  const connectionId = event.requestContext.connectionId;
  const body = event.body || "";
  console.log(`[DEFAULT] ${connectionId}: ${body}`);
  
  return {
    statusCode: 200,
    body: JSON.stringify({ message: "Unknown action", echo: body })
  };
};
