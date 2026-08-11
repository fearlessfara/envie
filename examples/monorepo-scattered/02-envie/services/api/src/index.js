// Application code lives beside its Terraform. Envie should ignore this.
exports.handler = async () => ({
  statusCode: 200,
  body: JSON.stringify({ ok: true }),
});
