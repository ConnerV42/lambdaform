// Shared utility layer
module.exports.formatResponse = (statusCode, body) => ({
  statusCode,
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body),
});

module.exports.getTimestamp = () => new Date().toISOString();
