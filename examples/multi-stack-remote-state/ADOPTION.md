# Adoption: hand-wired multi-stack remote state

Friction: **drop-in**.

`01-vanilla` is the usual “two roots + `terraform_remote_state`” layout. `02-envie` is that tree plus what `envie adopt` writes. The `.tf` files are identical, including the hardcoded remote-state key.

## Files added

| File | Role |
|------|------|
| `workspace.envie.yaml` | Shared backend, `prod` pinned to existing keys, ephemeral key pattern |
| `stacks/data/envie.yaml` | Unit `data`, no deps |
| `stacks/api/envie.yaml` | Unit `api`, `dependencies: [{ name: data }]` |

Adopt infers the edge because `data.terraform_remote_state.data`’s `key` equals the data stack’s backend `key`.

## Files removed

None. Do **not** delete `remote_state.tf`. Envie writes `envie_override.tf` that re-points the existing data source.

## Lines that must change

None, **if**:

- Each stack already names resources with `var.environment` (or another detected env variable)
- Remote-state `key` literals match the producer’s backend `key` (so adopt can wire deps)

## Lines that can stay

- Both `backend "s3"` blocks
- The entire `data.terraform_remote_state.data` block
- `data.terraform_remote_state.data.outputs.*` references in `main.tf`
- Lambda, IAM, HTTP API, DynamoDB

## What Envie generates at deploy time (gitignored)

In `stacks/api/`:

- `envie_override.tf` — same data source name, backend/key/workspace for the target env
- `envie.generated.tf` — `local.envie_*`

In `stacks/data/`:

- `envie.generated.tf` only (no remote state to override)

Deploying `--env pr-1` makes api read `envie/ephemeral/pr-1/stacks/data/terraform.tfstate` instead of `prod/data/terraform.tfstate`. Deploying `--env prod` keeps the original keys.

## Gaps

- Adopt matches a remote state to its producer by literal key first, then by the
  directory a relative `path` points into, then by data source name. A key built
  from variables (`key = "${var.env}/data/terraform.tfstate"`) therefore still
  wires up when the data source is named after the unit — but if it is named
  something else entirely, declare the dependency in `envie.yaml` yourself.
- Data source name vs unit name: if the block is `data.terraform_remote_state.database` but the unit is `data`, adopt writes `alias: database` so the override still hits the existing block.
- Vanilla apply order is manual (`data` then `api`). Envie’s planner deploys dependencies first.

## Reproduce

```bash
cp -R 01-vanilla /tmp/multistack-adopt
cd /tmp/multistack-adopt
envie adopt --name envie-test-multistack --environment prod
diff -rq . /path/to/examples/multi-stack-remote-state/02-envie
```
