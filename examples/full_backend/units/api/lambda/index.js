const { DynamoDBClient } = require('@aws-sdk/client-dynamodb');
const { DynamoDBDocumentClient, GetCommand, PutCommand, DeleteCommand, ScanCommand } = require('@aws-sdk/lib-dynamodb');

const client = new DynamoDBClient({});
const ddbDocClient = DynamoDBDocumentClient.from(client);

const TABLE_NAME = process.env.TABLE_NAME;

exports.handler = async (event) => {
  console.log('Event:', JSON.stringify(event, null, 2));

  const { httpMethod, pathParameters, body } = event;
  const id = pathParameters?.id;

  try {
    switch (httpMethod) {
      case 'POST':
        return await createItem(body);
      case 'GET':
        if (id) {
          return await getItem(id);
        }
        return await listItems();
      case 'PUT':
        return await updateItem(id, body);
      case 'DELETE':
        return await deleteItem(id);
      default:
        return response(405, { message: 'Method not allowed' });
    }
  } catch (error) {
    console.error('Error:', error);
    return response(500, {
      message: 'Internal server error',
      error: error.message
    });
  }
};

async function createItem(body) {
  const data = JSON.parse(body);

  if (!data.name) {
    return response(400, { message: 'Name is required' });
  }

  const item = {
    id: generateId(),
    name: data.name,
    description: data.description || '',
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString()
  };

  await ddbDocClient.send(new PutCommand({
    TableName: TABLE_NAME,
    Item: item
  }));

  return response(201, item);
}

async function getItem(id) {
  const result = await ddbDocClient.send(new GetCommand({
    TableName: TABLE_NAME,
    Key: { id }
  }));

  if (!result.Item) {
    return response(404, { message: 'Item not found' });
  }

  return response(200, result.Item);
}

async function listItems() {
  const result = await ddbDocClient.send(new ScanCommand({
    TableName: TABLE_NAME
  }));

  return response(200, {
    items: result.Items || [],
    count: result.Count
  });
}

async function updateItem(id, body) {
  const data = JSON.parse(body);

  // First check if item exists
  const existing = await ddbDocClient.send(new GetCommand({
    TableName: TABLE_NAME,
    Key: { id }
  }));

  if (!existing.Item) {
    return response(404, { message: 'Item not found' });
  }

  const updatedItem = {
    ...existing.Item,
    name: data.name || existing.Item.name,
    description: data.description !== undefined ? data.description : existing.Item.description,
    updatedAt: new Date().toISOString()
  };

  await ddbDocClient.send(new PutCommand({
    TableName: TABLE_NAME,
    Item: updatedItem
  }));

  return response(200, updatedItem);
}

async function deleteItem(id) {
  // First check if item exists
  const existing = await ddbDocClient.send(new GetCommand({
    TableName: TABLE_NAME,
    Key: { id }
  }));

  if (!existing.Item) {
    return response(404, { message: 'Item not found' });
  }

  await ddbDocClient.send(new DeleteCommand({
    TableName: TABLE_NAME,
    Key: { id }
  }));

  return response(200, { message: 'Item deleted successfully' });
}

function generateId() {
  return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

function response(statusCode, body) {
  return {
    statusCode,
    headers: {
      'Content-Type': 'application/json',
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Headers': 'Content-Type',
      'Access-Control-Allow-Methods': 'GET,POST,PUT,DELETE,OPTIONS'
    },
    body: JSON.stringify(body)
  };
}
