# Adoption: flat single-root static site

Friction: **drop-in**.

`01-vanilla` is a typical one-directory Terraform root. `02-envie` is that directory plus what `envie adopt` writes. The `.tf` files are identical.

## Files added

| File | Role |
|------|------|
| `workspace.envie.yaml` | Project name, S3 backend, `prod` (existing state) + ephemeral key pattern |
| `envie.yaml` | One unit (`root`) at `.` |
| `.gitignore` entries | `envie_override.tf`, `envie.generated.tf` |

## Files removed

None.

## Lines that must change

None, **if** names already include an environment variable Envie recognizes (`environment`, `env`, `stage`, …). This repo uses `var.environment`.

On deploy Envie sets `-var environment=prod` or `-var environment=pr-1`.

## Lines that can stay

- `provider "aws"` and `default_tags`
- The full `backend "s3"` block (same type → Envie leaves it and passes `-backend-config`)
- Every resource, data source, and output
- `www/index.html`

## What Envie generates at deploy time (gitignored)

- `envie.generated.tf` — `local.envie_*` (unused here; harmless)
- No `envie_override.tf` — nothing in this module needs re-pointing

## Gaps

- `envie adopt` names a flat root `root`. Fine, but not obvious.
- A module that hardcodes the bucket name (`bucket = "my-company-site"`) will clash across ephemeral envs. Adopt warns; it does not rewrite names.
- Backend bucket/table names stay as in the original `backend.tf`. Prefix them `envie-test-` in examples so leftovers are obvious.

## Reproduce

```bash
cp -R 01-vanilla /tmp/static-site-adopt
cd /tmp/static-site-adopt
envie adopt --name envie-test-static-site --environment prod
diff -rq . /path/to/examples/static-site/02-envie
```
