// Order fulfillment consumer
// Receives SNS messages from the orders topic and processes fulfillment

exports.handler = async (event) => {
  console.log('Fulfillment handler invoked');
  
  const results = [];
  
  for (const record of event.Records) {
    const message = JSON.parse(record.Sns.Message);
    const messageId = record.Sns.MessageId;
    const timestamp = record.Sns.Timestamp;
    
    console.log(`Processing order: ${JSON.stringify(message)}`);
    console.log(`SNS MessageId: ${messageId}, Timestamp: ${timestamp}`);
    
    // Simulate fulfillment processing
    const result = {
      orderId: message.orderId || 'unknown',
      status: 'fulfilled',
      service: process.env.SERVICE,
      processedAt: new Date().toISOString(),
      items: message.items || [],
    };
    
    results.push(result);
  }
  
  return {
    statusCode: 200,
    body: JSON.stringify({
      message: `Processed ${results.length} orders for fulfillment`,
      results,
    }),
  };
};
