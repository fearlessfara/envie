# Static site (after Envie)

Same Terraform as [`../01-vanilla`](../01-vanilla). Envie was added with `envie adopt` — no `.tf` edits.

```bash
# From the envie repo root
cargo build --release
ENVIE=../../../target/release/envie

# What adopt would write (already present here)
# $ENVIE adopt --name envie-test-static-site --environment prod

$ENVIE deploy --env prod --dry-run
$ENVIE deploy --env pr-1 --dry-run

# Apply (creates the state bucket/table if missing, then the site)
aws-vault exec personal --no-session -- $ENVIE deploy --env pr-1 --no-prompt

# Tear down
aws-vault exec personal --no-session -- $ENVIE delete --env pr-1 --no-prompt
```

Envie injects `-var environment=<env>` because this module already declares `variable "environment"`. Resource names stay unique across ephemeral environments without touching `main.tf`.
