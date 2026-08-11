// Intentionally SDK-free so the zip has no node_modules.
// TABLE_NAME comes from the data stack via terraform_remote_state.
exports.handler = async () => ({
  statusCode: 200,
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ table: process.env.TABLE_NAME }),
});
