// Token Authorizer Lambda
// Validates Bearer token and returns IAM policy

exports.handler = async (event) => {
  console.log('Authorizer event:', JSON.stringify(event, null, 2));

  const token = event.authorizationToken || event.authorization_token;
  const expectedToken = process.env.AUTH_TOKEN || 'super-secret-token-123';
  const methodArn = event.methodArn || event.method_arn || 'arn:aws:execute-api:*:*:*';

  // Strip "Bearer " prefix if present
  const cleanToken = token ? token.replace(/^Bearer\s+/i, '') : '';

  if (cleanToken === expectedToken) {
    console.log('Token valid — allowing');
    return generatePolicy('user', 'Allow', methodArn, {
      userId: '12345',
      role: 'admin',
    });
  }

  console.log('Token invalid — denying');
  return generatePolicy('user', 'Deny', methodArn);
};

function generatePolicy(principalId, effect, resource, context) {
  const policy = {
    principalId,
    policyDocument: {
      Version: '2012-10-17',
      Statement: [
        {
          Action: 'execute-api:Invoke',
          Effect: effect,
          Resource: resource,
        },
      ],
    },
  };

  if (context) {
    policy.context = context;
  }

  return policy;
}
