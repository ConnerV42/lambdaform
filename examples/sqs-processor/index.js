// Order Processor — SQS-triggered Lambda
// Processes order messages from the orders queue.
// Demonstrates batch processing, partial failures, and error handling.

exports.handler = async (event) => {
  console.log(`Processing ${event.Records.length} order(s)`);

  const results = [];
  const batchItemFailures = [];

  for (const record of event.Records) {
    const messageId = record.messageId;
    console.log(`Processing message ${messageId}`);

    try {
      const order = JSON.parse(record.body);

      // Validate order
      if (!order.orderId || !order.items || !Array.isArray(order.items)) {
        throw new Error(`Invalid order format: missing orderId or items`);
      }

      // Calculate total
      const total = order.items.reduce((sum, item) => {
        return sum + (item.price * (item.quantity || 1));
      }, 0);

      console.log(`Order ${order.orderId}: ${order.items.length} items, total $${total.toFixed(2)}`);

      // Simulate processing
      const processed = {
        orderId: order.orderId,
        status: "processed",
        total: total,
        itemCount: order.items.length,
        processedAt: new Date().toISOString(),
      };

      results.push(processed);

      // Log SQS metadata
      console.log(`  Source: ${record.eventSourceARN}`);
      console.log(`  Sent at: ${record.attributes.SentTimestamp}`);
      console.log(`  Receive count: ${record.attributes.ApproximateReceiveCount}`);

    } catch (err) {
      console.error(`Failed to process message ${messageId}: ${err.message}`);
      // Report partial batch failure — SQS will retry just this message
      batchItemFailures.push({ itemIdentifier: messageId });
    }
  }

  console.log(`Processed: ${results.length}, Failed: ${batchItemFailures.length}`);

  // Return partial batch failure response (SQS feature)
  return {
    statusCode: 200,
    batchItemFailures: batchItemFailures,
    body: JSON.stringify({
      processed: results.length,
      failed: batchItemFailures.length,
      results: results,
    }),
  };
};
