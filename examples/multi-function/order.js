const { successResponse, errorResponse, logInfo, validateEnv } = require('common');

// In-memory order store
const orders = new Map([
  ['1001', { id: '1001', userId: '1', product: 'Laptop', quantity: 1, total: 1299.99, status: 'shipped' }],
  ['1002', { id: '1002', userId: '2', product: 'Mouse', quantity: 2, total: 39.98, status: 'delivered' }],
  ['1003', { id: '1003', userId: '1', product: 'Keyboard', quantity: 1, total: 149.99, status: 'pending' }],
]);

exports.handler = async (event) => {
  try {
    // Validate required environment variables
    validateEnv(['API_VERSION', 'SERVICE_NAME', 'ENVIRONMENT', 'MAX_ORDERS']);

    logInfo('Processing request', {
      method: event.httpMethod,
      path: event.path,
      maxOrders: process.env.MAX_ORDERS
    });

    // List all orders
    if (event.httpMethod === 'GET' && event.path === '/orders') {
      const allOrders = Array.from(orders.values());
      const maxOrders = parseInt(process.env.MAX_ORDERS, 10);

      return successResponse({
        orders: allOrders.slice(0, maxOrders),
        count: allOrders.length,
        limit: maxOrders
      }, 'Orders retrieved successfully');
    }

    // Route not found
    return errorResponse('Route not found', 404);

  } catch (error) {
    console.error('Error:', error);
    return errorResponse(error.message, 500, { stack: error.stack });
  }
};
