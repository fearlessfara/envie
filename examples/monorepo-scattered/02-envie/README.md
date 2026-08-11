# Envie: Terraform scattered through a monorepo

Same Terraform as `01-vanilla`, plus `workspace.envie.yaml` and one `envie.yaml`
per root module. No `.tf` file differs, and `scripts/deploy.sh` still works.

`envie adopt` walks the whole repository and sorts out what is deployable:

| Directory | Verdict |
|-----------|---------|
| `platform/network` | Unit `network` |
| `services/api/terraform` | Unit `api` |
| `modules/naming` | Not deployable — used as a child module by both |
| `services/api/src`, `scripts` | Not Terraform |

The unit is called `api`, not `terraform`. A trailing `terraform/` directory says
nothing about which unit it is, so it is dropped from the name.

## Ordering and wiring

The `terraform_remote_state` block in the API stack is matched to the network
unit by the bucket and key it already names, so the dependency is inferred rather
than declared:

```bash
envie deploy --env prod --dry-run
```

`network` is planned first, then `api`, reading `network` from `stable.prod` at
the state path it already uses.

## New environments

```bash
envie deploy --env pr-1
```

Both stacks get fresh state under `envie/ephemeral/pr-1/`, and the API's remote
state block is repointed at the `pr-1` copy of the network rather than the real
one. That repointing happens in a generated `envie_override.tf`, which is
gitignored; the original block is untouched.

Verified against AWS: in `pr-1` the API's `vpc-id` parameter holds the id from
the `pr-1` network, while production's still holds production's.

Each unit gets its own variable name: `environment=pr-1` for the network stack
and `env=pr-1` for the API stack.
