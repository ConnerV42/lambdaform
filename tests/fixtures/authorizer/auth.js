// Token-based Lambda authorizer
// Returns Allow if Authorization header contains "Bearer valid-token"
// Returns Deny otherwise
exports.handler = async (event, context) => {
  const token = event.authorizationToken || '';
  
  if (token === 'Bearer valid-token') {
    return {
      principalId: 'user123',
      policyDocument: {
        Version: '2012-10-17',
        Statement: [
          {
            Action: 'execute-api:Invoke',
            Effect: 'Allow',
            Resource: event.methodArn,
          },
        ],
      },
      context: {
        userId: 'user123',
        role: 'admin',
      },
    };
  }

  return {
    principalId: 'anonymous',
    policyDocument: {
      Version: '2012-10-17',
      Statement: [
        {
          Action: 'execute-api:Invoke',
          Effect: 'Deny',
          Resource: event.methodArn,
        },
      ],
    },
  };
};
