exports.handler = async (event) => {
  console.log('Checking inventory:', JSON.stringify(event));
  
  // Simulate inventory check
  const inStock = event.items.every(item => {
    // Items with quantity > 100 are "out of stock"
    return item.quantity <= 100;
  });
  
  return {
    ...event,
    inStock,
    inventoryCheckedAt: new Date().toISOString()
  };
};
