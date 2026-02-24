// Alert handler consumer
// Receives SNS messages from the system-alerts topic

exports.handler = async (event) => {
  console.log('Alert handler invoked');
  
  const processed = [];
  
  for (const record of event.Records) {
    const message = JSON.parse(record.Sns.Message);
    const subject = record.Sns.Subject || 'No subject';
    const topicArn = record.Sns.TopicArn;
    
    console.log(`Alert received - Subject: ${subject}`);
    console.log(`Topic: ${topicArn}`);
    console.log(`Alert body: ${JSON.stringify(message)}`);
    
    // Simulate alert processing
    const result = {
      alertId: message.alertId || 'unknown',
      severity: message.severity || 'info',
      service: process.env.SERVICE,
      acknowledged: true,
      processedAt: new Date().toISOString(),
    };
    
    processed.push(result);
  }
  
  return {
    statusCode: 200,
    body: JSON.stringify({
      message: `Processed ${processed.length} alerts`,
      alerts: processed,
    }),
  };
};
