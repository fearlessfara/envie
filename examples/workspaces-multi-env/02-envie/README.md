# Workspaces multi-env (after Envie)

The same SQS queues as [`../01-vanilla`](../01-vanilla), and the same Terraform.
Every `.tf` file here is byte-for-byte identical to the vanilla version; the only
additions are `workspace.envie.yaml` and `envie.yaml`.

Envie keeps using the mechanism the repository already had. `terraform.workspace`
still names the queues, `workspace_key_prefix` still separates the state, and each
environment is still a Terraform workspace — Envie just creates and selects them.

```bash
cargo build --release
ENVIE=../../../target/release/envie

# Produced by:
#   envie adopt --name envie-test-workspaces \
#     --environment prod --environment staging --environment dev

$ENVIE deploy --env prod --dry-run      # workspace prod, key terraform.tfstate
$ENVIE deploy --env pr-1 --dry-run      # workspace pr-1, same key

aws-vault exec personal --no-session -- $ENVIE deploy --env pr-1
aws-vault exec personal --no-session -- $ENVIE delete --env pr-1 --no-prompt
```

`deploy --env pr-1` creates queues named `envie-test-jobs-pr-1` and state at
`env/pr-1/terraform.tfstate` — indistinguishable from what
`terraform workspace new pr-1 && terraform apply` would have produced.

See [`../ADOPTION.md`](../ADOPTION.md) for what adoption decided and why.
