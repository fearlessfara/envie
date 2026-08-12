<p align="center">
  <img src="assets/logo.png" alt="Envie" width="160" />
</p>

# Envie

Envie gives an existing Terraform repository as many environments as you want,
without reorganising it.

Point it at a repository you already have, and it works out which directories are
deployable, how they depend on each other, and where their state lives. From then
on the same code can be deployed as your existing environment, as a throwaway
environment per pull request, or as any mixture of the two — one unit reading
production's state while the rest is a fresh copy.

Envie never edits your Terraform. It reads it, and adds files of its own.

## Install

```bash
brew install fearlessfara/tap/envie
```

Homebrew warns that the tap is not trusted, because it is not an official one.
To silence that on every later upgrade, `brew trust --tap fearlessfara/tap`.

Or build from source:

```bash
cargo build --release
# the binary is at target/release/envie
```

Requires Terraform 1.0+ and, for S3 state, the AWS CLI. Envie does not handle
credentials itself: it shells out to `terraform` and `aws`, which use the standard
credential chain.

## GitHub Actions

A composite Action at the root of this repository installs the matching release
binary and puts `envie` on `PATH`. Pin it to the same tag as the CLI:

```yaml
- uses: fearlessfara/envie@v0.2.1
- run: envie deploy --env pr-${{ github.event.pull_request.number }} --no-prompt
```

Omit `version` to use the tag you pinned (`@v0.2.1`); set `version: latest` to
always take the newest release. Pass `args` to run a command in the same step:

```yaml
- uses: fearlessfara/envie@v0.2.1
  with:
    args: delete --env pr-${{ github.event.pull_request.number }} --no-prompt
```

The Action does not install Terraform or configure cloud credentials. Use
[`hashicorp/setup-terraform`](https://github.com/hashicorp/setup-terraform) and
your usual AWS setup (for example
[`aws-actions/configure-aws-credentials`](https://github.com/aws-actions/configure-aws-credentials)
with OIDC) before calling Envie.

Ephemeral environments per pull request:

```yaml
name: Envie

on:
  pull_request:
    types: [opened, synchronize, reopened, closed]

permissions:
  contents: read
  id-token: write

jobs:
  deploy:
    if: github.event.action != 'closed'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: hashicorp/setup-terraform@v4
        with:
          terraform_wrapper: false
      - uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: arn:aws:iam::123456789012:role/envie-ci
          aws-region: eu-west-1
      - uses: fearlessfara/envie@v0.2.1
      - run: envie deploy --env pr-${{ github.event.pull_request.number }} --no-prompt

  destroy:
    if: github.event.action == 'closed'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: hashicorp/setup-terraform@v4
        with:
          terraform_wrapper: false
      - uses: aws-actions/configure-aws-credentials@v4
        with:
          role-to-assume: arn:aws:iam::123456789012:role/envie-ci
          aws-region: eu-west-1
      - uses: fearlessfara/envie@v0.2.1
        with:
          args: delete --env pr-${{ github.event.pull_request.number }} --no-prompt
```

## Releasing

Same steps every time: changelog, version bump, tag. The checklist, Homebrew
secret, and the pre-release dogfood (dry-run, or apply and delete in the same
session) are in [CONTRIBUTING.md](CONTRIBUTING.md).

## Adopting a repository you already have

```bash
cd my-terraform-repo
envie adopt --dry-run          # what Envie makes of the repository
envie adopt --environment prod # write the configuration
```

Adoption reports what it found before writing anything:

```
Root modules (2)
  platform/network             0 resource(s), 2 output(s)
                                 backend: s3 → acme-state/legacy/platform/network.tfstate
  services/api                 2 resource(s), 2 output(s)
                                 backend: s3 → acme-state/legacy/services/api.tfstate
                                 depends on network (data.terraform_remote_state.network)

Not deployable (1)
  modules/param                used as a child module by platform/network

Environments
  prod         adopts the state 2 unit(s) already have, in workspace 'default'
  ephemeral    one per feature or pull request, isolated by state path and workspace
```

It works out:

- **Which directories are deployable.** Any directory with `.tf` files that no
  other module uses as a child module. A shared `modules/` tree is not an
  environment; a root module is.
- **What depends on what**, from the `terraform_remote_state` data sources already
  in the code — matched by state path, by relative path, or by name.
- **Where your state already is.** The environment you adopt keeps pointing at the
  exact state paths the repository uses, in Terraform's `default` workspace, so
  your first deploy against it is a no-op rather than a second copy of production.
  This includes state paths kept outside the Terraform: a `backend "s3" {}` block
  filled in at init time from a `.tfbackend` or backend `.hcl` file is read too,
  and where there is one such file per environment, every one of them is adopted.
- **Which variable names your environment.** If the code has a `var.environment`,
  `var.env`, `var.stage` or `var.name_prefix`, Envie feeds the environment name
  into it, so a new environment does not fight the old one for resource names.
  Environments that already exist are the exception: whatever the repository
  already says they are called — a variable default, or a var file of their own —
  is what built them, so Envie leaves it alone rather than renaming anything.

Layout does not matter. A single `main.tf` at the root, `live/` plus `modules/`,
`envs/dev` and `envs/prod` copied side by side, or roots scattered anywhere — all
are adopted as they are. Copied environment directories stay separate units;
nothing is collapsed and no state is moved. Because they build the same resources,
build new environments from one of them at a time
(`envie deploy --env pr-42 --unit dev`); adoption points this out when it sees
that layout.

Neither does the way you already separate environments:

- **By state path** (`prod/api/terraform.tfstate`) — the adopted environment pins
  those exact paths, and new environments get paths of their own.
- **By Terraform workspace** (`terraform.workspace` in resource names, with
  `workspace_key_prefix` on the backend) — Envie keeps the workspace name equal to
  the environment name, so the adopted environment stays in the workspace its
  state is in, and `envie deploy --env pr-42` produces exactly what
  `terraform workspace new pr-42` would have.

Adoption writes `workspace.envie.yaml`, an `envie.yaml` per root module, and
`.gitignore` entries. Nothing else.

## Everyday use

```bash
# Your existing environment, now managed by Envie. Should report no changes.
envie deploy --env prod --dry-run
envie deploy --env prod

# A complete throwaway copy, from the same code.
envie deploy --env pr-42

# Just the API, reading production's network instead of its own.
envie deploy --env pr-42 --unit api -E network:stable.prod

# Tear it down. No flags needed: Envie replays how it was deployed.
envie destroy --env pr-42   # remove the infrastructure
envie delete --env pr-42    # remove the infrastructure and its state
```

`delete` refuses to touch a long-lived environment, and never removes the state
bucket or lock table — those are shared, and on an adopted repository they are
yours rather than Envie's.

## Environments

An environment is either **stable** — declared in `workspace.envie.yaml`, meant to
last — or **ephemeral**, created on demand from an id you choose.

References work anywhere an environment is named:

| Reference | Means |
| --- | --- |
| `prod` | the stable `prod` if declared, otherwise an ephemeral environment called `prod` |
| `stable.prod` | the stable `prod`, unambiguously |
| `ephemeral.pr-42` | the ephemeral environment `pr-42` |
| `ephemeral` | the environment currently being deployed |

State is isolated by path and by Terraform workspace:

```
envie/ephemeral/{id}/{unit_path}/terraform.tfstate     # ephemeral
envie/{environment}/{unit_path}/terraform.tfstate      # stable
```

An adopted environment overrides this with the literal paths it already had,
recorded as `state_keys` in `workspace.envie.yaml`.

### Which environments exist

`envie list` answers that. The long-lived ones come from `workspace.envie.yaml`,
but an ephemeral environment exists only because somebody deployed it — often
somebody else, from another machine — so those are read back out of the
deployment records in the state backend:

```
📋 Environments in acme

Long-lived
  prod   deployed 2026-08-11 22:03 UTC (api, network)
         workspace default, state in s3://acme-state

Ephemeral
  pr-42  deployed 2026-08-11 21:40 UTC (api)
         workspace acme-pr-42, state in s3://acme-state
```

`--json` gives the same answer to a pipeline deciding whether an environment is
still up. Nothing is deployed, destroyed or initialised to produce this list, and
a backend Envie cannot read is reported rather than quietly left out.

## What Envie writes into your modules

Two files per root module, regenerated on every deploy and safe to delete:

- **`envie_override.tf`** — a Terraform [override
  file](https://developer.hashicorp.com/terraform/language/files/override), which
  repoints blocks your code already has. This is how an existing
  `terraform_remote_state` block reads a different environment without your code
  changing.
- **`envie.generated.tf`** — blocks your code does not have: remote state data
  sources for declared dependencies, and `local.envie_*` values such as
  `envie_environment_id` and `envie_common_tags`.

Backends are configured through `terraform init -backend-config`, so an existing
backend block keeps working as written.

Envie also records what each deploy did, in the state backend under
`envie/manifests/{environment}.json`: which units were deployed, and which
environment each of their dependencies was read from. That is what lets
`envie destroy` reproduce a deployment nobody remembers the flags for, and what
`envie list` reads to find environments that exist but are declared nowhere.
Tearing an environment down takes it back out of the record, so a destroyed
environment stops being reported as deployed.

## Commands

| Command | Purpose |
| --- | --- |
| `envie adopt` | Turn an existing repository into an Envie project |
| `envie init` | Start a new project from scratch |
| `envie deploy --env <id>` | Deploy an environment |
| `envie destroy --env <id>` | Destroy an environment's infrastructure |
| `envie delete --env <id>` | Destroy an ephemeral environment and remove its state |
| `envie list` | Show which environments exist, declared and deployed |
| `envie show` | Show units and their dependencies |
| `envie output --env <id>` | Show an environment's Terraform outputs (`--format` table/json/yaml/env); see [examples/api-output-export](examples/api-output-export/) |
| `envie generate --env <id>` | Fill a `.env` template from an environment's outputs |
| `envie clean` | Remove generated files |

Most commands take `--dry-run` to show the plan, `--unit` to narrow to one unit,
and `-E unit:environment` to redirect a dependency.

## Documentation

- [QUICKSTART.md](QUICKSTART.md) — building a project from scratch
- [ENVIRONMENT_OVERRIDES.md](ENVIRONMENT_OVERRIDES.md) — mixing environments with `-E`
- [examples/](examples/) — six repository layouts, each as plain Terraform and
  again after adoption, with an `ADOPTION.md` recording exactly what changed
- [CHANGELOG.md](CHANGELOG.md) — what shipped in each version
- [CONTRIBUTING.md](CONTRIBUTING.md) — releasing and dogfooding
- [LICENSE](LICENSE) — MIT
