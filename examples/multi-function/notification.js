const { successResponse, errorResponse, logInfo, logDebug, validateEnv } = require('common');

// In-memory notification log
const notifications = [];

exports.handler = async (event) => {
  try {
    // Validate required environment variables
    validateEnv(['API_VERSION', 'SERVICE_NAME', 'ENVIRONMENT', 'EMAIL_ENABLED', 'SMS_ENABLED']);

    logInfo('Processing notification request', {
      method: event.httpMethod,
      path: event.path
    });

    // Send notification
    if (event.httpMethod === 'POST' && event.path === '/notifications') {
      const body = event.body ? JSON.parse(event.body) : null;

      if (!body || !body.message) {
        return errorResponse('Missing required field: message', 400);
      }

      const notification = {
        id: notifications.length + 1,
        message: body.message,
        recipient: body.recipient || 'default@example.com',
        type: body.type || 'email',
        timestamp: new Date().toISOString(),
        channels: {
          email: process.env.EMAIL_ENABLED === 'true',
          sms: process.env.SMS_ENABLED === 'true'
        }
      };

      logDebug('Notification details', notification);

      notifications.push(notification);

      // Simulate sending via enabled channels
      const sent = [];
      if (notification.channels.email) {
        sent.push('email');
        logInfo('Email sent', { to: notification.recipient });
      }
      if (notification.channels.sms) {
        sent.push('sms');
        logInfo('SMS sent', { to: notification.recipient });
      }

      if (sent.length === 0) {
        return errorResponse('No notification channels enabled', 400, {
          EMAIL_ENABLED: process.env.EMAIL_ENABLED,
          SMS_ENABLED: process.env.SMS_ENABLED
        });
      }

      return successResponse({
        notification,
        sentVia: sent
      }, 'Notification sent successfully');
    }

    // Route not found
    return errorResponse('Route not found', 404);

  } catch (error) {
    console.error('Error:', error);
    return errorResponse(error.message, 500, { stack: error.stack });
  }
};
