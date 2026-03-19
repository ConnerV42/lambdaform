/**
 * Echo event handler — returns the full event for assertion testing.
 */
exports.handler = async (event, context) => {
  return {
    statusCode: 200,
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(event),
  };
};
