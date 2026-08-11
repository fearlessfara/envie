# Envie Quickstart Guide

Two ways in, depending on what you have. If you already have Terraform, adopt it —
that takes minutes and builds nothing. If you don't, the second half of this guide
scaffolds a project you can deploy and tear down for free, to see the workflow
before writing any Terraform of your own.

## Prerequisites

- Envie installed: `brew install fearlessfara/tap/envie`, or `cargo build --release`
- Terraform 1.0+
- AWS credentials in your shell, and the AWS CLI for S3 state. Envie shells out to
  `terraform` and `aws`, which use the standard credential chain — it never reads
  credentials itself.

## Adopting an existing repository

Adoption is the fastest route to multiple environments, because it starts from the
Terraform you already have. It does not reorganise your repository, does not edit
your `.tf` files, and does not move your state.

### 1. See what Envie makes of it

```bash
cd my-terraform-repo
envie adopt --dry-run
```

Read the report before going further. It lists the directories Envie will treat as
deployable units, the ones it will ignore and why, the dependencies it inferred
from your `terraform_remote_state` blocks, and the state paths it found. If a
directory is missing or misclassified, that is the thing to fix first.

### 2. Adopt it

```bash
envie adopt --environment production
```

`--environment` names the environment your existing infrastructure becomes. Pick
the name you already call it — `prod`, `production`, `live`. Envie writes:

- `workspace.envie.yaml`, with your existing state paths recorded under
  `state_keys` so that environment keeps managing the resources you already have
- an `envie.yaml` in each root module, with the dependencies it inferred
- `.gitignore` entries for the files Envie generates

Nothing else is touched. If Envie configuration already exists, adoption stops
unless you pass `--force`.

### 3. Prove the adoption is a no-op

This is the step worth being careful about. Deploying the adopted environment
should change nothing at all:

```bash
envie deploy --env production --dry-run   # check the state paths look right
envie deploy --env production             # expect "no changes" from Terraform
```

If Terraform proposes to create resources that already exist, the state paths in
`workspace.envie.yaml` are wrong. Fix `state_keys` rather than applying.

### 4. Create environments

```bash
# A complete, isolated copy of everything.
envie deploy --env pr-42

# Or just one unit, reading production's state for the rest.
envie deploy --env pr-42 --unit api -E network:stable.production
```

The ephemeral environment gets its own state paths and its own Terraform
workspace, and the environment name is fed into your repository's own environment
variable (`var.environment`, `var.env`, `var.stage`, ...) so resource names don't
collide with production's.

### 5. Throw it away

```bash
envie delete --env pr-42
```

No flags needed, even for the `-E` deployment above: Envie recorded how the
environment was deployed and replays it. `delete` removes the environment's
infrastructure and state, and refuses to touch a stable environment. The state
bucket and lock table are never deleted.

---

## Starting from scratch

The rest of this guide builds a project with `envie init`. Everything below is the
actual output of the commands, in order — the scaffolded units use Terraform's
built-in `terraform_data`, so you can deploy and destroy real environments without
creating anything that costs money.

### 1. Create the project

```bash
mkdir myapp && cd myapp
envie init --name myapp --description "My application infrastructure"
```

```
✅ Created an Envie project in /path/to/myapp

  workspace.envie.yaml   project, environments and backend
  units/db/              a unit that produces an output
  units/api/             a unit that reads it

The two units use Terraform's built-in terraform_data, so they cost
nothing to deploy. With AWS credentials in your shell:

  envie deploy --env pr-1 --dry-run   # what would happen
  envie deploy --env pr-1             # build a whole environment
  envie output --env pr-1             # see what it produced
  envie delete --env pr-1             # remove it again
```

That is the whole project:

```
workspace.envie.yaml
units/db/envie.yaml
units/db/main.tf
units/api/envie.yaml
units/api/main.tf
.gitignore
README.md
```

A **unit** is a Terraform root module with its own state — any directory with an
`envie.yaml` next to its `.tf` files. An **environment** is a complete set of those
units, deployed together with state of its own.

### 2. Read the configuration

`workspace.envie.yaml` describes the environments and where their state goes:

```yaml
version: "1.0"

project:
  name: myapp
  description: My application infrastructure

environments:
  # One short-lived environment per feature, pull request or experiment.
  # Created by deploying to any name not listed under stable.
  ephemeral:
    naming_pattern: "{project}-{id}"
    key_pattern: "envie/ephemeral/{id}/{unit_path}/terraform.tfstate"
    backend: &backend
      type: s3
      config:
        # S3 bucket names are global, so this may need a suffix of your own.
        # Envie offers to create the bucket and the lock table on first deploy.
        bucket: myapp-tfstate
        dynamodb_table: myapp-tflocks
        encrypt: "true"
        region: eu-west-1

  stable:
    prod:
      description: The long-lived environment
      workspace: default
      backend: *backend
      key_pattern: "envie/prod/{unit_path}/terraform.tfstate"
```

**Check the bucket name before your first deploy** — S3 bucket names are global,
so `myapp-tfstate` may well be taken.

Ephemeral environments are not listed anywhere: deploying to any name that is not
under `stable` creates one. Each gets its own state path (from `key_pattern`, where
`{id}` is the environment name and `{unit_path}` the unit's directory) and its own
Terraform workspace (from `naming_pattern`). `prod` uses Terraform's `default`
workspace, which is what a repository that never used workspaces already has.

Each unit's `envie.yaml` says what it needs:

```yaml
name: api
description: Stands in for an API. Reads the db unit's output.
unit_type: service
state_management: dedicated

# Envie turns each of these into a terraform_remote_state data source
# pointing at the environment being deployed. Override one with
# -E <unit>:<environment> to read from somewhere else.
dependencies:
  - name: db
```

- `name` is how everything else refers to this unit: `--unit api`, `-E api:...`.
- `dependencies` are other units, by `name` (or by `path` for a directory that has
  no `envie.yaml` name yet). They fix the deployment order and become remote state
  data sources.
- `unit_type` (`service`, `module`, `component`, `layer`, `application`) and
  `state_management` (`dedicated` by default) are descriptive; `dedicated` means
  the unit gets a state file of its own, which is almost always what you want.

`envie show` prints the same thing for the whole project, from anywhere inside it:

```
📋 Units in myapp

  api   units/api
        Stands in for an API. Reads the db unit's output.
        reads db
  db    units/db
        Stands in for a database. Produces a name other units read.

Environments: envie list
```

`envie show --unit api` adds what reads it, which is the question worth asking
before changing a unit:

```
📦 api

  Stands in for an API. Reads the db unit's output.

  path     units/api
  type     service
  state    its own state file
  reads    db (units/db)
  read by  nothing
```

### 3. See the plan before running it

```bash
envie deploy --env pr-1 --dry-run
```

```
📋 Deployment plan (dry run)

Environment: pr-1 (ephemeral)
Workspace:   myapp-pr-1
Backend:     s3

1. db
   path:  units/db
   state: envie/ephemeral/pr-1/units/db/terraform.tfstate

2. api
   path:  units/api
   state: envie/ephemeral/pr-1/units/api/terraform.tfstate
   reads: db from ephemeral.pr-1
          envie/ephemeral/pr-1/units/db/terraform.tfstate

2 unit(s) would be deployed.
```

`pr-1` is not declared anywhere, so it is an ephemeral environment. `db` is planned
first because `api` reads it, and `api` reads the copy of `db` in its own
environment.

This runs no Terraform and needs no credentials, so it is worth reading whenever
you are unsure what a command will do.

### 4. Deploy it

```bash
envie deploy --env pr-1
```

The first deploy offers to create the state bucket and lock table:

```
🏗️  Backend Infrastructure Setup
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📦 S3 Bucket to create:
   Name: myapp-tfstate
   Region: eu-west-1
   Purpose: Terraform state storage

🔒 DynamoDB Table to create:
   Name: myapp-tflocks
   Region: eu-west-1
   Purpose: Terraform state locking
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🚀 Deploying 2 unit(s) to pr-1

[1/2] db
  📍 units/db
  💾 envie/ephemeral/pr-1/units/db/terraform.tfstate
  ⚡ terraform apply
  ✅ done

[2/2] api
  📍 units/api
  💾 envie/ephemeral/pr-1/units/api/terraform.tfstate
  ⚡ terraform apply
  ✅ done

✅ pr-1 is deployed.
```

If the bucket already exists — because a colleague deployed first, or you are
adopting a repository — nothing is created and the deploy goes straight ahead.

### 5. See what it produced

```bash
envie output --env pr-1
```

```
📊 Outputs for pr-1

┌─ api ─────────────────────────────────
│  endpoint: https://pr-1.example.internal
│  reads_table: myapp-pr-1-items
└────────────────────────────────────────

┌─ db ─────────────────────────────────
│  table_name: myapp-pr-1-items
└────────────────────────────────────────
```

Note `reads_table`: `api` was wired to `pr-1`'s own table, not to production's.

To turn those outputs into a `.env` for an application, write a template naming
the outputs you want as `unit.output`:

```bash
cat > .env.example <<'EOF'
API_URL=api.endpoint
TABLE_NAME=db.table_name
LOG_LEVEL=debug
EOF

envie generate --env pr-1
```

```
API_URL="https://pr-1.example.internal"
TABLE_NAME="myapp-pr-1-items"
LOG_LEVEL="debug"
```

Values that don't match a `unit.output` reference, like `LOG_LEVEL`, are copied
through as written.

### 6. Look at what Envie wrote into the units

```bash
cat units/api/envie.generated.tf
```

```hcl
# Managed by Envie - do not edit, regenerated on every deploy.

terraform {
  backend "s3" {}
}

data "terraform_remote_state" "db" {
  backend   = "s3"
  workspace = "myapp-pr-1"

  config = {
    bucket         = "myapp-tfstate"
    dynamodb_table = "myapp-tflocks"
    encrypt        = "true"
    key            = "envie/ephemeral/pr-1/units/db/terraform.tfstate"
    region         = "eu-west-1"
  }
}

locals {
  envie_project_name   = "myapp"
  envie_environment_id = "pr-1"
  envie_unit_name      = "api"
  envie_workspace      = "myapp-pr-1"

  envie_common_tags = {
    Project     = "myapp"
    Environment = "pr-1"
    Unit        = "api"
    ManagedBy   = "envie"
  }
}
```

This is why `units/api/main.tf` can read `data.terraform_remote_state.db` and name
resources after `local.envie_environment_id` without naming an environment
anywhere. Deploy a different environment and the same file is rewritten to point
at that environment's state.

You may also see `envie_override.tf`. Envie writes that one only when your code
already declares a block it needs to change — a Terraform
[override file](https://developer.hashicorp.com/terraform/language/files/override)
is how an existing `terraform_remote_state` block gets repointed at another
environment without your code changing. The scaffolded units declare no such
blocks, so only `envie.generated.tf` appears here.

Both files are regenerated on every deploy, gitignored, and safe to delete —
`envie clean` does it for you.

### 7. Deploy the long-lived environment too

```bash
envie deploy --env prod
```

Same code, different environment. `prod` is declared under `stable`, so it goes to
`envie/prod/...` in Terraform's `default` workspace, and its resources are named
after `prod` rather than `pr-1`.

```bash
envie list
```

```
📋 Environments in myapp

Long-lived
  prod  deployed 2026-08-11 22:22 UTC (api, db)
        workspace default, state in s3://myapp-tfstate
        The long-lived environment

Ephemeral
  pr-1  deployed 2026-08-11 22:21 UTC (api, db)
        workspace myapp-pr-1, state in s3://myapp-tfstate
```

The long-lived environments come from `workspace.envie.yaml`; the ephemeral ones
are found in the deployment records Envie keeps in the state backend, so they show
up here even when somebody else deployed them from another machine. `--json` gives
a script the same answer.

### 8. Mix environments

The point of all this is being able to build part of an environment and borrow the
rest. Deploy only `api`, reading production's `db`:

```bash
envie deploy --env pr-2 --unit api -E db:stable.prod --dry-run
```

```
📋 Deployment plan (dry run)

Environment: pr-2 (ephemeral)
Workspace:   myapp-pr-2
Backend:     s3

1. api
   path:  units/api
   state: envie/ephemeral/pr-2/units/api/terraform.tfstate
   reads: db from stable.prod (overridden)
          envie/prod/units/db/terraform.tfstate
```

`db` is not deployed at all, and `api` reads production's state instead of a `db`
of its own. Envie records the override, so tearing `pr-2` down later needs no
flags.

### 9. Tear it down

```bash
envie delete --env pr-1
```

```
Step 1: destroying infrastructure

  api
    ✅ destroyed

  db
    ✅ destroyed

Step 2: removing state

✅ 'pr-1' is gone. The state backend was left untouched.
```

Units are destroyed in reverse dependency order, and `envie list` stops reporting
the environment. `delete` refuses to touch a long-lived environment, and never
removes the state bucket or lock table — those are shared.

For a long-lived environment, `destroy` removes the infrastructure but keeps the
environment declared and its state file in place:

```bash
envie destroy --env prod --dry-run   # see the order first
envie destroy --env prod             # asks you to type the environment name
```

Finally, `envie clean` removes the generated files from every unit:

```
units/api
  removed envie.generated.tf
units/db
  removed envie.generated.tf

✅ Removed 2 file(s).
```

## Common workflows

### One environment per pull request

```bash
envie deploy --env pr-42          # build it
envie generate --env pr-42        # point the app at it
envie delete --env pr-42          # when the PR closes
```

### Iterating on one unit against shared dependencies

```bash
# Build only the API, reading production's database.
envie deploy --env pr-42 --unit api -E db:stable.prod

# Edit units/api/main.tf, then redeploy just that unit.
envie deploy --env pr-42 --unit api
```

Faster than rebuilding everything, and the override is replayed on teardown.

### From inside a unit directory

```bash
cd units/api
envie deploy --env pr-42
```

Envie finds the project root and works out which unit you are in, so `--unit` is
optional.

## Troubleshooting

**`no workspace.envie.yaml found in ... or any parent directory`**
You are outside an Envie project. Run `envie adopt` in a Terraform repository, or
`envie init` in an empty directory.

**`unknown stable environment 'stagng' (declared stable environments: prod)`**
A typo, or an environment you haven't declared. Bare names that are not declared
are treated as ephemeral ids, so `--env stagng` would otherwise have quietly built
a new environment; `stable.` prefixes are checked.

**Terraform proposes to create resources that already exist**
The state path is wrong for that environment. On an adopted repository, check
`state_keys` in `workspace.envie.yaml` against the state paths your repository
actually uses, and do not apply until a dry run is clean.

**`could not read s3://.../envie/manifests/: Unable to locate credentials`**
`envie list` reports this rather than pretending the list is complete. Declared
environments are still listed; the ephemeral ones need credentials for the bucket.

**A deploy left resources behind after failing**
Deploys are not transactional: units already applied stay applied. Fix the problem
and deploy again, or tear the environment down with `envie delete --env <id>`.

## Next steps

- [README.md](README.md) — what Envie generates, how state and dependencies are
  resolved, and the full command list
- [ENVIRONMENT_OVERRIDES.md](ENVIRONMENT_OVERRIDES.md) — `-E` in more depth
- [examples/](examples/) — six real repository layouts, each with a `vanilla`
  version and the same repository after adoption
- Issues and questions: <https://github.com/fearlessfara/envie/issues>
