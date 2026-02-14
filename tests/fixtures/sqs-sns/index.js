exports.handler = async (event) => {
  console.log('Received event:', JSON.stringify(event, null, 2));
  
  const records = event.Records || [];
  const source = records[0]?.eventSource || records[0]?.EventSource || 'unknown';
  
  return {
    statusCode: 200,
    body: JSON.stringify({
      message: `Processed ${records.length} record(s) from ${source}`,
      recordCount: records.length,
    }),
  };
};
