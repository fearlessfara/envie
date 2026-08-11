# Multi-stack remote state (after Envie)

Same stacks as [`../01-vanilla`](../01-vanilla). The hand-written `data.terraform_remote_state.data` block stays; Envie overrides its backend config per environment.

```bash
cargo build --release
ENVIE=../../../target/release/envie

$ENVIE deploy --env prod --dry-run
$ENVIE deploy --unit api --env pr-1 --dry-run

aws-vault exec personal --no-session -- $ENVIE deploy --env pr-1 --no-prompt
aws-vault exec personal --no-session -- $ENVIE delete --env pr-1 --no-prompt
```

`--unit api` also deploys `data` because Envie walks dependencies.
