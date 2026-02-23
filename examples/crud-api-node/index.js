// In-memory data store (persists across warm invocations)
const items = new Map();
let nextId = 1;

// Initialize with some sample data
if (items.size === 0) {
  items.set('1', { id: '1', name: 'Sample Item 1', description: 'First sample item', createdAt: new Date().toISOString() });
  items.set('2', { id: '2', name: 'Sample Item 2', description: 'Second sample item', createdAt: new Date().toISOString() });
  nextId = 3;
}

exports.handler = async (event) => {
  console.log('Received event:', JSON.stringify(event, null, 2));
  
  const { httpMethod, path, pathParameters, body } = event;
  
  try {
    // Route to appropriate handler
    if (path === '/items' && httpMethod === 'GET') {
      return listItems();
    }
    
    if (path === '/items' && httpMethod === 'POST') {
      return createItem(body);
    }
    
    if (path.match(/^\/items\/[^/]+$/) && httpMethod === 'GET') {
      return getItem(pathParameters.id);
    }
    
    if (path.match(/^\/items\/[^/]+$/) && httpMethod === 'PUT') {
      return updateItem(pathParameters.id, body);
    }
    
    if (path.match(/^\/items\/[^/]+$/) && httpMethod === 'DELETE') {
      return deleteItem(pathParameters.id);
    }
    
    // No matching route
    return {
      statusCode: 404,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Not Found', path, method: httpMethod })
    };
    
  } catch (error) {
    console.error('Error:', error);
    return {
      statusCode: 500,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Internal Server Error', message: error.message })
    };
  }
};

// List all items
function listItems() {
  const allItems = Array.from(items.values());
  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ 
      items: allItems,
      count: allItems.length 
    })
  };
}

// Create a new item
function createItem(bodyString) {
  if (!bodyString) {
    return {
      statusCode: 400,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Bad Request', message: 'Request body is required' })
    };
  }
  
  const data = JSON.parse(bodyString);
  
  if (!data.name) {
    return {
      statusCode: 400,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Bad Request', message: 'name field is required' })
    };
  }
  
  const id = String(nextId++);
  const item = {
    id,
    name: data.name,
    description: data.description || '',
    createdAt: new Date().toISOString()
  };
  
  items.set(id, item);
  
  return {
    statusCode: 201,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(item)
  };
}

// Get a specific item
function getItem(id) {
  const item = items.get(id);
  
  if (!item) {
    return {
      statusCode: 404,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Not Found', message: `Item ${id} not found` })
    };
  }
  
  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(item)
  };
}

// Update an item
function updateItem(id, bodyString) {
  const item = items.get(id);
  
  if (!item) {
    return {
      statusCode: 404,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Not Found', message: `Item ${id} not found` })
    };
  }
  
  if (!bodyString) {
    return {
      statusCode: 400,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Bad Request', message: 'Request body is required' })
    };
  }
  
  const data = JSON.parse(bodyString);
  
  const updatedItem = {
    ...item,
    name: data.name !== undefined ? data.name : item.name,
    description: data.description !== undefined ? data.description : item.description,
    updatedAt: new Date().toISOString()
  };
  
  items.set(id, updatedItem);
  
  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(updatedItem)
  };
}

// Delete an item
function deleteItem(id) {
  const item = items.get(id);
  
  if (!item) {
    return {
      statusCode: 404,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: 'Not Found', message: `Item ${id} not found` })
    };
  }
  
  items.delete(id);
  
  return {
    statusCode: 200,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message: 'Item deleted successfully', id })
  };
}
