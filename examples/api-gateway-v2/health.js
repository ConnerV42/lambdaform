// Simple health check handler — tests multi-function routing with v2
exports.handler = async (event) => {
  console.log("Health check — v2 event version:", event.version);

  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      status: "healthy",
      timestamp: new Date().toISOString(),
      eventVersion: event.version || "unknown",
      routeKey: event.routeKey,
    }),
  };
};
