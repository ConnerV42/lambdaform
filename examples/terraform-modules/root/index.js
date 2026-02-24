exports.handler = async (event) => {
  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      message: "Root-level handler",
      path: event.path,
      env: process.env.ENV || "unknown",
    }),
  };
};
