exports.handler = async (event) => {
  const { httpMethod } = event;
  return {
    statusCode: 200,
    body: JSON.stringify({ method: httpMethod, message: `${httpMethod} request successful` })
  };
};
