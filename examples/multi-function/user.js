const { successResponse, errorResponse, logInfo, validateEnv } = require('common');

// In-memory user store
const users = new Map([
  ['1', { id: '1', name: 'Alice Johnson', email: 'alice@example.com', role: 'admin' }],
  ['2', { id: '2', name: 'Bob Smith', email: 'bob@example.com', role: 'user' }],
  ['3', { id: '3', name: 'Charlie Brown', email: 'charlie@example.com', role: 'user' }],
]);

exports.handler = async (event) => {
  try {
    // Validate required environment variables
    validateEnv(['API_VERSION', 'SERVICE_NAME', 'ENVIRONMENT']);

    logInfo('Processing request', {
      method: event.httpMethod,
      path: event.path,
      apiVersion: process.env.API_VERSION
    });

    // List all users
    if (event.httpMethod === 'GET' && event.path === '/users') {
      const allUsers = Array.from(users.values());
      return successResponse({
        users: allUsers,
        count: allUsers.length
      }, 'Users retrieved successfully');
    }

    // Route not found
    return errorResponse('Route not found', 404);

  } catch (error) {
    console.error('Error:', error);
    return errorResponse(error.message, 500, { stack: error.stack });
  }
};
