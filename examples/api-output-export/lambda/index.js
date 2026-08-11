// Intentionally SDK-free so the zip has no node_modules.
exports.handler = async () => ({
  statusCode: 200,
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ ok: true, greeting: "envie" }),
});
