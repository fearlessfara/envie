# Adoption: Terraform scattered through a monorepo

Friction: **drop-in**.

## Files added

| File | Role |
|------|------|
| `workspace.envie.yaml` | Project name, S3 backend, `prod` (existing state) + ephemeral key pattern |
| `platform/network/envie.yaml` | Unit `network` |
| `services/api/terraform/envie.yaml` | Unit `api`, depending on `network` |
| `.gitignore` entries | `envie_override.tf`, `envie.generated.tf` |

## Files removed

None. Both backend blocks, the `terraform_remote_state` block, and
`scripts/deploy.sh` all stay.

## Lines that must change

None.

## What this case exercises

**Finding the root modules.** Nothing about the layout is regular: one root is
two levels down under `platform/`, the other is three levels down under
`services/api/`, and they are mixed in with application code. A directory is a
root module when no other module uses it as a child, which is what separates
`platform/network` from `modules/naming`.

**Naming the units.** `services/api/terraform` becomes `api`. Trailing
`terraform`, `tf`, `infra`, `infrastructure`, `iac` and `deploy` segments are
dropped, since they describe the file layout rather than the unit. Had there been
a second `<service>/terraform`, both would have kept enough of the path to stay
distinct.

**Inferring the dependency.** The API's `terraform_remote_state` block names
`envie-test-monorepo-tfstate` and `platform/network/terraform.tfstate`, which is
exactly where the network unit keeps its state. That match is what produces
`dependencies: [network]`, and it is why the block does not have to be rewritten:
per environment, Envie overrides it to point at that environment's copy.

**Two names for the same idea.** `var.environment` in one stack and `var.env` in
the other. The variable is detected per unit, so each gets the one it declares.
`var.env` has no default, so the adopted environment does pass `env=prod` — the
same value `scripts/deploy.sh` was passing.

## Gaps

- A registry module (`source = "terraform-aws-modules/…"`) is correctly ignored,
  but a module sourced from a git URL that happens to also exist in the
  repository is not matched up.
- Dependency inference is by literal bucket and key. A `terraform_remote_state`
  block whose key is built from an expression is not matched, and the dependency
  has to be written into `envie.yaml` by hand.

## Reproduce

```bash
cp -R 01-vanilla /tmp/monorepo-adopt
cd /tmp/monorepo-adopt
envie adopt --name envie-test-monorepo --environment prod
diff -rq . /path/to/examples/monorepo-scattered/02-envie
```
