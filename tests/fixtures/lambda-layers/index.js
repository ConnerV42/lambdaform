// Lambda function that uses a layer
const utils = require('utils');

exports.handler = async (event, context) => {
  const greeting = utils.greet("World");
  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      message: greeting,
      layerVersion: utils.version,
    }),
  };
};
