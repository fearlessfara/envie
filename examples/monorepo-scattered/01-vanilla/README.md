# Vanilla: Terraform scattered through a monorepo

Terraform is not the point of this repository, so it is wherever each team put
it:

```text
platform/network/            root module, owns the shared network
services/api/terraform/      root module, beside the service it deploys
services/api/src/            application code
modules/naming/              shared local module, not deployable on its own
scripts/deploy.sh            loops over the roots in order
```

The two roots are wired together by hand: `services/api` reads
`platform/network`'s outputs through a `terraform_remote_state` data source with
the bucket and key written out literally.

They also disagree about what to call the environment. The network stack takes
`var.environment` and the API stack takes `var.env`, which is why `deploy.sh`
passes both to both.

## Deploying it by hand

```bash
./scripts/deploy.sh prod
```

A second environment means new state keys for both roots, a matching change to
the hardcoded key inside the `terraform_remote_state` block, and hoping nobody
runs the script against the wrong one.

## Resources

Four SSM parameters, all free. The network stack stands in for a VPC rather than
creating one, so nothing here costs anything to leave running.
