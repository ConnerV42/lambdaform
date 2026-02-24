// $disconnect handler — called when a client disconnects
exports.handler = async (event) => {
  const connectionId = event.requestContext.connectionId;
  console.log(`[DISCONNECT] ${connectionId}`);
  
  // In production, you'd remove connectionId from DynamoDB
  return { statusCode: 200, body: "Disconnected" };
};
