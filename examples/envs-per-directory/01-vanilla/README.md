# Vanilla: one directory per environment

The most common way a Terraform repository grows a second environment: copy the
first one.

```text
envs/dev/     backend key dev/terraform.tfstate,  var.environment defaults to "dev"
envs/prod/    backend key prod/terraform.tfstate, var.environment defaults to "prod"
modules/worker/   the code both of them call
```

Each directory is a root module with its own state. The only differences between
them are the backend key and the default of `var.environment`.

## Deploying it by hand

```bash
cd envs/prod
terraform init
terraform apply
```

A third environment means copying a directory again, and remembering to change
the backend key inside it. Nothing stops you from copying `envs/prod` and leaving
`key = "prod/terraform.tfstate"` in place, which points the new directory at
production's state.

## Resources

An SQS queue and an SSM parameter per environment, both free.
