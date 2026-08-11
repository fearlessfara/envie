# Environment overrides

A unit reads its dependencies from the environment being deployed. `-E` changes
that for one dependency, so an environment can be part new and part borrowed:
your API on a branch, reading production's database.

```bash
envie deploy --env pr-42 --unit api -E db:stable.prod
```

## What a dependency resolves to

Dependencies are declared by unit name in each unit's `envie.yaml`, and nothing
there names an environment:

```yaml
name: api
dependencies:
  - name: db
```

That is deliberate. The same file has to work for every environment, so `db`
means "the `db` of whichever environment is being deployed" until something says
otherwise. `-E` is that something, and it lives on the command line rather than in
the repository because it describes one deployment, not the design of the project.

## The syntax

`-E <unit>:<environment>`, repeatable. The unit is the dependency's unit name —
the `name` in its `envie.yaml`, which is also what `envie show` lists. The
environment is any reference Envie understands:

| Reference | Means |
| --- | --- |
| `stable.prod` | the declared long-lived environment `prod`, unambiguously |
| `prod` | `stable.prod` if it is declared, otherwise an ephemeral environment called `prod` |
| `ephemeral.pr-7` | the ephemeral environment `pr-7` |
| `ephemeral` | the environment currently being deployed, which is the default |

```bash
# Read production's database and network, deploy only the API.
envie deploy --env pr-42 --unit api \
  -E db:stable.prod \
  -E network:stable.prod

# Read another branch's database.
envie deploy --env pr-42 --unit api -E db:ephemeral.pr-7
```

## What actually happens

Ask before deploying, and Envie prints exactly what it will wire up:

```bash
envie deploy --env pr-2 --unit api -E db:stable.prod --dry-run
```

```
Environment: pr-2 (ephemeral)
Workspace:   myapp-pr-2
Backend:     s3

1. api
   path:  units/api
   state: envie/ephemeral/pr-2/units/api/terraform.tfstate
   reads: db from stable.prod (overridden)
          envie/prod/units/db/terraform.tfstate
```

Two things to notice. The `terraform_remote_state` data source for `db` is
generated pointing at production's state path and workspace, so `api` sees
production's outputs. And `db` is not in the plan at all — an overridden
dependency is read, not deployed, which is the point: nothing rebuilds the part
you are borrowing.

The state Envie reads is somebody else's, so it is read-only. What your Terraform
then *does* with those outputs is not: if `api` writes to the table name it read
from production, it is writing to production's table. Overrides are for reading
shared infrastructure, and it is worth being deliberate about which direction the
data flows.

## Tearing down

Envie records the overrides a deploy used, in `envie/manifests/{environment}.json`
in the state backend. Teardown replays them:

```bash
envie delete --env pr-42     # no -E needed
```

This matters more than it looks. Destroying `api` requires evaluating its
configuration, which still refers to `db`; pointed at an empty `db` state instead
of production's, Terraform cannot evaluate what it is removing. Passing the wrong
`-E` on teardown, or none at all, is how an environment becomes stuck — so Envie
remembers rather than asking you to.

## When to reach for it

Overrides earn their keep when a dependency is slow, expensive, or holds data
worth keeping: a database with a useful dataset, a VPC, anything with an hourly
charge. Build those once as a long-lived environment and borrow them.

When a change touches the dependency itself, do not override — deploy the whole
environment and let it build its own copy:

```bash
envie deploy --env pr-42          # every unit, wired to itself
```

That is also the safer default. An environment that reads only itself can be
destroyed in any order and cannot affect anything else.
