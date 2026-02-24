exports.handler = async (event) => {
  console.log('Validating order:', JSON.stringify(event));
  
  const { orderId, amount, items } = event;
  
  if (!orderId) {
    throw new Error('ValidationError: Missing orderId');
  }
  
  const minAmount = parseInt(process.env.MIN_ORDER_AMOUNT || '10');
  if (!amount || amount < minAmount) {
    const err = new Error(`ValidationError: Order amount ${amount} below minimum ${minAmount}`);
    err.name = 'ValidationError';
    throw err;
  }
  
  if (!items || items.length === 0) {
    const err = new Error('ValidationError: No items in order');
    err.name = 'ValidationError';
    throw err;
  }
  
  return {
    ...event,
    validated: true,
    validatedAt: new Date().toISOString()
  };
};
