# Adoption: backend supplied at init time

Friction: **drop-in**.

## Files added

| File | Role |
|------|------|
| `workspace.envie.yaml` | Project name, S3 backend read from `config/*.s3.tfbackend`, `prod` and `staging` with their real state paths and var files, ephemeral key pattern |
| `envie.yaml` | One unit (`root`) at `.` |
| `.gitignore` entries | `envie_override.tf`, `envie.generated.tf` |

## Files removed

None. `backend "s3" {}` stays empty, and both `.tfbackend` files stay where they
are — you can still run raw Terraform against this repository exactly as before.

## Lines that must change

None.

## What makes this case different

The backend block is empty, so scanning the `.tf` files alone finds a repository
with an S3 backend and no idea which bucket. Adoption reads the settings files
too: anything ending `.tfbackend`, and `.hcl` files whose name mentions the
backend, in the module directory or in a subdirectory of it like `config/`.

Because there is one file per environment, adoption gets more than the
environment you name first. `--environment prod --environment staging` adopts
both, each pinned to the key its own file gives:

```yaml
prod:
  state_keys:
    ".": prod/terraform.tfstate
  var_files:
    - config/prod.tfvars

staging:
  state_keys:
    ".": staging/terraform.tfstate
  var_files:
    - config/staging.tfvars
```

Both carry `keep_repository_defaults: true`, so Envie does not pass
`-var environment=…` over the top of the var file the repository already has.

## Gaps

- The file for an environment is found by name: `config/prod.s3.tfbackend`
  matches `prod`. A repository that names them `config/1-production.hcl` while
  calling the environment `prod` needs the state path pinning by hand.
- Only literal values are read. A settings file is plain `key = value` in
  practice, so this has not been a problem, but an expression would be skipped.
- Adoption does not check that the bucket in the file exists. A stale settings
  file is adopted as-is and fails at `terraform init`.

## Reproduce

```bash
cp -R 01-vanilla /tmp/partial-adopt
cd /tmp/partial-adopt
envie adopt --name envie-test-partial --environment prod --environment staging
diff -rq . /path/to/examples/partial-backend-config/02-envie
```
