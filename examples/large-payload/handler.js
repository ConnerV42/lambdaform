// Text echo handler — receives JSON body, returns info about the payload
exports.handler = async (event) => {
  const body = event.body || '';
  const isBase64 = event.isBase64Encoded || false;
  
  // Decode if base64
  const decoded = isBase64 ? Buffer.from(body, 'base64') : Buffer.from(body);
  
  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      receivedBytes: decoded.length,
      isBase64Encoded: isBase64,
      bodyPreview: decoded.toString('utf8').substring(0, 100),
      headers: event.headers || {},
    }),
  };
};
