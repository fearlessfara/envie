# Adoption: native Terraform workspaces

Friction: **drop-in**.

This is the pattern that overlaps most with Envie: the vanilla repository already
separates environments, using `terraform workspace` plus `workspace_key_prefix`,
and already names resources from `terraform.workspace`.

Envie recognises that and adopts it as it is. It does not replace the mechanism —
it keeps using it, and adds environments on top.

## Files added

| File | Role |
|------|------|
| `workspace.envie.yaml` | One stable environment per Terraform workspace, plus ephemeral |
| `envie.yaml` | One unit (`root`) |

```bash
envie adopt --environment prod --environment staging --environment dev
```

The first `--environment` is the one whose state Envie checks against; the rest are
declared alongside it. Each maps to the Terraform workspace of the same name.

## Files removed or edited

None. No `.tf` file changes at all.

## What adoption produces

```yaml
environments:
  ephemeral:
    naming_pattern: "{id}"                 # the workspace IS the environment id
    key_pattern: "terraform.tfstate"       # unchanged; the prefix separates them
    backend:
      type: s3
      config:
        workspace_key_prefix: env          # kept, because it is where state already is

  stable:
    prod:
      workspace: prod                      # not 'default' — that would rename everything
      key_pattern: "terraform.tfstate"
```

Two decisions matter here, and both come from the repository rather than from
Envie:

1. **`workspace_key_prefix` is kept.** It is part of where the existing state
   actually is (`env/prod/terraform.tfstate`). Dropping it would move the state.
2. **The workspace name equals the environment name.** Since resource names are
   built from `terraform.workspace`, adopting into `default` would rename every
   resource — a destroy-and-recreate of production.

## Existing state

Nothing is migrated, because nothing needs to be. `envie deploy --env prod` selects
workspace `prod`, reads `env/prod/terraform.tfstate`, and reports **no changes**.

A new environment lands exactly where a hand-run `terraform workspace new pr-9`
would have put it:

```
env/prod/terraform.tfstate     # untouched
env/pr-9/terraform.tfstate     # created by envie deploy --env pr-9
```

and its resources are named `envie-test-jobs-pr-9`, following the repository's own
convention — not Envie's.

## Naming

`terraform.workspace` keeps working and needs no replacement. Adoption reports
`terraform.workspace already varies per environment` rather than asking for a
`var.environment`.

Adding `variable "environment"` is still an option if you want the code to be
independent of workspaces, but it buys nothing here.

## Gaps

- Adoption does not check which Terraform workspaces actually exist; it trusts the
  names you pass and tells you to confirm with `terraform workspace list`.
- A single state key shared by every environment means every environment is locked
  through the same DynamoDB lock id per workspace — the same as the vanilla setup,
  so no regression, but parallel deploys of *the same* environment still serialise.

## Reproduce

Verified end to end against real AWS (SQS queues, free tier):

```bash
cp -R 01-vanilla /tmp/ws-e2e && cd /tmp/ws-e2e
# create the state bucket and lock table first, then:
terraform init && terraform workspace new prod && terraform apply

envie adopt --environment prod
envie deploy --env prod          # No changes. Infrastructure matches.
envie deploy --env pr-9          # creates envie-test-jobs-pr-9 in env/pr-9/...
envie delete --env pr-9          # removes only pr-9; prod state untouched
envie destroy --env prod
```
