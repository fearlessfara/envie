# Adoption: one directory per environment

Friction: **drop-in, with one thing to know**.

## Files added

| File | Role |
|------|------|
| `workspace.envie.yaml` | Project name, S3 backend, `prod` and `dev` environments, ephemeral key pattern |
| `envs/dev/envie.yaml` | Unit `dev` |
| `envs/prod/envie.yaml` | Unit `prod` |
| `.gitignore` entries | `envie_override.tf`, `envie.generated.tf` |

## Files removed

None. Both `backend "s3"` blocks stay exactly as they are.

## Lines that must change

None.

## The thing to know

A directory per environment gives Envie two units that build the same resources.
That shapes two behaviours:

**Deploying the adopted environment sets no variables.** `var.environment` has a
default in each directory, and for the environment being adopted that default is
the answer the repository already gave — it is what built the live resources.
Envie writes `keep_repository_defaults: true` on that environment and leaves the
defaults alone. Without it, `envie deploy --env prod` would run the `dev` unit
with `environment=prod` against `dev/terraform.tfstate`, renaming dev's queue to
production's name. Every other environment still gets the name injected, which is
what keeps them apart.

**New environments take one unit at a time.** `envie deploy --env pr-1` with no
`--unit` would build `dev` and `prod` into `pr-1` from the same module, and they
would fight over `envie-test-envdirs-pr-1-jobs`. `envie adopt` prints this when it
sees sibling directories named after environments, and suggests the `--unit` form.

## Gaps

- Envie does not collapse the two directories into one unit with two
  environments. That is the tidier end state, and it needs a state move, so it is
  not something adoption should do behind your back.
- The suggestion to use `--unit` is advice, not enforcement. Deploying every unit
  into one new environment is still allowed, and Terraform will fail on the name
  clash rather than Envie catching it first.

## Reproduce

```bash
cp -R 01-vanilla /tmp/envdirs-adopt
cd /tmp/envdirs-adopt
envie adopt --name envie-test-envdirs --environment prod --environment dev
diff -rq . /path/to/examples/envs-per-directory/02-envie
```
