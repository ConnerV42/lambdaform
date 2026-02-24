// Binary handler — accepts binary data, returns binary response
exports.handler = async (event) => {
  const body = event.body || '';
  const isBase64 = event.isBase64Encoded || false;
  
  // Decode input
  const inputBuffer = isBase64 ? Buffer.from(body, 'base64') : Buffer.from(body);
  
  // Create a reversed copy as "processing"
  const reversed = Buffer.from(inputBuffer).reverse();
  
  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/octet-stream' },
    isBase64Encoded: true,
    body: reversed.toString('base64'),
  };
};
