// API Gateway v2 (HTTP API) handler
// Receives APIGatewayProxyEventV2 format
const items = new Map();
let nextId = 1;

// Seed some data
items.set("1", { id: "1", name: "First item", status: "active" });
items.set("2", { id: "2", name: "Second item", status: "pending" });
nextId = 3;

exports.handler = async (event) => {
  // Log the v2 event structure for verification
  console.log("Event version:", event.version);
  console.log("Route key:", event.routeKey);
  console.log("Raw path:", event.rawPath);
  console.log("Request context:", JSON.stringify(event.requestContext?.http));

  // v2 event format uses routeKey and requestContext.http.method
  const method = event.requestContext?.http?.method || event.httpMethod;
  const path = event.rawPath || event.path;
  const pathParams = event.pathParameters || {};

  // Verify v2-specific fields exist
  if (!event.version) {
    console.warn("WARNING: event.version is missing — may not be v2 format");
  }
  if (event.version && event.version !== "2.0") {
    console.warn(`WARNING: expected version 2.0, got ${event.version}`);
  }

  try {
    // Route handling
    if (method === "GET" && path === "/items") {
      return respond(200, { items: Array.from(items.values()), count: items.size });
    }

    if (method === "POST" && path === "/items") {
      const body = parseBody(event);
      const id = String(nextId++);
      const item = { id, ...body, createdAt: new Date().toISOString() };
      items.set(id, item);
      return respond(201, item);
    }

    if (method === "GET" && pathParams.id) {
      const item = items.get(pathParams.id);
      if (!item) return respond(404, { error: "Item not found" });
      return respond(200, item);
    }

    if (method === "PUT" && pathParams.id) {
      const existing = items.get(pathParams.id);
      if (!existing) return respond(404, { error: "Item not found" });
      const body = parseBody(event);
      const updated = { ...existing, ...body, updatedAt: new Date().toISOString() };
      items.set(pathParams.id, updated);
      return respond(200, updated);
    }

    if (method === "DELETE" && pathParams.id) {
      if (!items.has(pathParams.id)) return respond(404, { error: "Item not found" });
      items.delete(pathParams.id);
      return respond(204, null);
    }

    return respond(404, { error: "Not found", path, method });
  } catch (err) {
    console.error("Error:", err);
    return respond(500, { error: err.message });
  }
};

function parseBody(event) {
  if (!event.body) return {};
  const raw = event.isBase64Encoded
    ? Buffer.from(event.body, "base64").toString()
    : event.body;
  return JSON.parse(raw);
}

function respond(statusCode, body) {
  // v2 format: can return simple object (auto-wrapped) or structured response
  return {
    statusCode,
    headers: { "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : "",
  };
}
