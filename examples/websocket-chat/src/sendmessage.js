// sendmessage handler — broadcasts message via @connections API
const http = require("http");

exports.handler = async (event) => {
  const connectionId = event.requestContext.connectionId;
  const body = JSON.parse(event.body || "{}");
  const message = body.data || "empty";
  
  console.log(`[SENDMESSAGE] from ${connectionId}: ${message}`);
  
  // Use @connections API to send message back to the sender
  // In production this would be ApiGatewayManagementApi
  const connectionsUrl = process.env.CONNECTIONS_URL || "http://localhost:3001";
  
  const payload = JSON.stringify({ from: connectionId, message });
  
  await postToConnection(connectionsUrl, connectionId, payload);
  
  return { statusCode: 200, body: "Sent" };
};

function postToConnection(baseUrl, connectionId, data) {
  return new Promise((resolve, reject) => {
    const url = new URL(`/@connections/${connectionId}`, baseUrl);
    const req = http.request(url, { method: "POST", headers: { "Content-Type": "application/json" } }, (res) => {
      let body = "";
      res.on("data", (chunk) => body += chunk);
      res.on("end", () => {
        console.log(`@connections POST ${connectionId} → ${res.statusCode}`);
        resolve({ statusCode: res.statusCode, body });
      });
    });
    req.on("error", (e) => {
      console.error(`@connections POST failed: ${e.message}`);
      resolve({ statusCode: 500 }); // don't reject, just log
    });
    req.write(data);
    req.end();
  });
}
