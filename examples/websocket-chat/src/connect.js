// $connect handler — called when a client connects
exports.handler = async (event) => {
  const connectionId = event.requestContext.connectionId;
  console.log(`[CONNECT] ${connectionId}`);
  
  // In production, you'd store connectionId in DynamoDB
  return { statusCode: 200, body: "Connected" };
};
