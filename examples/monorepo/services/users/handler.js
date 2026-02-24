// Users service — Node.js handlers

exports.list_users = async (event) => {
  const users = [
    { id: "u1", name: "Alice", email: "alice@example.com" },
    { id: "u2", name: "Bob", email: "bob@example.com" },
    { id: "u3", name: "Charlie", email: "charlie@example.com" },
  ];

  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      users,
      count: users.length,
      table: process.env.TABLE_NAME,
    }),
  };
};

exports.get_user = async (event) => {
  const userId = event.pathParameters?.userId || "unknown";
  return {
    statusCode: 200,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      id: userId,
      name: "Alice",
      email: "alice@example.com",
      table: process.env.TABLE_NAME,
    }),
  };
};
