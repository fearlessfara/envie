# Envie: one directory per environment

Same Terraform as `01-vanilla`, plus `workspace.envie.yaml` and one `envie.yaml`
per directory. No `.tf` file differs.

Envie adopts the directories where they are. `envs/dev` and `envs/prod` become
two units, `dev` and `prod`, each keeping the state it already has. No state is
moved and no resource is renamed.

## The adopted environment changes nothing

```bash
envie deploy --env prod --dry-run
```

Both units are planned against their existing state with no variables set. That
is deliberate: each directory already says which environment it is, and setting
`environment=prod` on the `dev` unit would rename dev's queue to production's
name while writing to dev's state.

## New environments come from one directory

```bash
envie deploy --env pr-1 --unit dev
```

This copies the `dev` directory's code into a new environment: fresh state under
`envie/ephemeral/pr-1/`, and `environment=pr-1` so the queue is
`envie-test-envdirs-pr-1-jobs`.

Deploy one unit at a time here. Without `--unit`, both `dev` and `prod` would be
built into `pr-1` from the same code, and they would collide over every name.
`envie adopt` says so when it finds this layout.

## Tearing it down

```bash
envie delete --env pr-1
```

Only `pr-1` is removed. `dev` and `prod` keep their state, which Envie never
derived and so never deletes.
