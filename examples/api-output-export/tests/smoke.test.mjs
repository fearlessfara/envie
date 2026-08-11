import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const outputsDir = process.env.OUTPUTS_DIR ?? join(import.meta.dirname, "..");
const endpoint = process.env.API_API_ENDPOINT;

test("envie wrote outputs.json and outputs.yaml", () => {
  assert.ok(
    existsSync(join(outputsDir, "outputs.json")),
    `missing ${join(outputsDir, "outputs.json")}`,
  );
  assert.ok(
    existsSync(join(outputsDir, "outputs.yaml")),
    `missing ${join(outputsDir, "outputs.yaml")}`,
  );
});

test("API_API_ENDPOINT is set from envie output --format env", () => {
  assert.ok(endpoint, "API_API_ENDPOINT must be set (source the exported .env)");
  assert.match(endpoint, /^https:\/\//);
});

test("GET api_endpoint returns the Lambda greeting", async () => {
  assert.ok(endpoint, "API_API_ENDPOINT must be set");

  const response = await fetch(endpoint);
  assert.equal(response.status, 200);

  const body = await response.json();
  assert.deepEqual(body, { ok: true, greeting: "envie" });
});
