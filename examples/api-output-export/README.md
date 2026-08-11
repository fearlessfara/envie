# API output export

Envie-native demo: deploy an HTTP API + Lambda, export Terraform outputs as
`.env` / JSON / YAML with `envie output`, then run Node smoke tests against the
live endpoint.

## What it creates

Free-tier friendly resources in `eu-west-1`, named `envie-test-api-export-…`:

- Lambda (`nodejs20.x`) returning `{ ok: true, greeting: "envie" }`
- API Gateway HTTP API pointed at that Lambda
- IAM role + CloudWatch logs

One-time remote state (created by `run.sh` if missing):

- S3 bucket `envie-test-api-export-tfstate`
- DynamoDB table `envie-test-api-export-tflocks` (on-demand)

## Run

From the repo root, build Envie, then:

```bash
cargo build --release
cd examples/api-output-export
chmod +x run.sh
aws-vault exec personal --no-session -- env ENVIE=../../target/release/envie ./run.sh demo-1
```

`run.sh` will:

1. Bootstrap the state bucket/lock table if needed
2. `envie deploy --env demo-1`
3. Export all three formats:
   - `envie output --env demo-1 --format env -f .env`
   - `envie output --env demo-1 --format json -f outputs.json`
   - `envie output --env demo-1 --format yaml -f outputs.yaml`
4. Source `.env` and run `npm test` in `tests/`
5. `envie destroy --env demo-1 --no-prompt`

Always destroy in the same session. Delete the bootstrap bucket and lock table
yourself when you are finished experimenting.

## Expected `.env`

Unit name is `api`, so keys are `UNIT_OUTPUT`:

```text
API_API_ENDPOINT="https://….execute-api.eu-west-1.amazonaws.com"
API_FUNCTION_NAME="envie-test-api-export-demo-1-api"
```

Resource names use `--env` as `var.environment` (here `demo-1`).

## Validate without AWS

```bash
cd examples/api-output-export
terraform init -backend=false
terraform validate
```
