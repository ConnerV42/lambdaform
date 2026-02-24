exports.handler = async (event) => {
  console.log('Notifying customer:', JSON.stringify(event));
  
  const orderId = event.orderId || 'unknown';
  
  return {
    orderId,
    notificationType: 'order_confirmation',
    channel: 'email',
    sentAt: new Date().toISOString(),
    message: `Your order ${orderId} has been confirmed and is being shipped!`
  };
};
