exports.handler = async (event) => {
  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      message: "List items (depth-2 module)",
      table: process.env.TABLE_NAME,
      env: process.env.ENV,
      items: [
        { id: 1, name: "Item A" },
        { id: 2, name: "Item B" },
      ],
    }),
  };
};
