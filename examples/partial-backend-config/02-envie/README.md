# Envie: backend supplied at init time

Same Terraform as `01-vanilla`, plus `workspace.envie.yaml` and `envie.yaml`. The
empty `backend "s3" {}` block and both `.tfbackend` files stay exactly as they
are.

`envie adopt` reads `config/*.s3.tfbackend`, so it knows where each environment's
state is even though the Terraform does not say. Both `prod` and `staging` are
adopted with their real state paths and their own var files:

```bash
envie deploy --env prod --dry-run      # state prod/terraform.tfstate,    config/prod.tfvars
envie deploy --env staging --dry-run   # state staging/terraform.tfstate, config/staging.tfvars
```

Neither has `environment` injected. The repository already answers that question
in its var file, and `-var` would silently win over `-var-file`.

## New environments

```bash
envie deploy --env pr-1
```

Fresh state under `envie/ephemeral/pr-1/`, `environment=pr-1`, and `log_level`
left at its default. There is no `pr-1.tfbackend` and no `pr-1.tfvars` to write:
the pair of files per environment is exactly the overhead Envie removes.
