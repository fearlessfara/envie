# Vanilla: backend supplied at init time

One root module, several environments, and a `backend "s3" {}` block with nothing
in it. The bucket and key come from a file chosen at init:

```text
backend.tf                    backend "s3" {}   — empty on purpose
config/prod.s3.tfbackend      bucket + key for prod
config/staging.s3.tfbackend   bucket + key for staging
config/prod.tfvars            environment = "prod",    log_level = "warn"
config/staging.tfvars         environment = "staging", log_level = "debug"
```

## Deploying it by hand

```bash
terraform init -reconfigure -backend-config=config/prod.s3.tfbackend
terraform apply -var-file=config/prod.tfvars
```

Switching environments means `-reconfigure` and remembering to change both flags
together. Passing `config/prod.s3.tfbackend` with `config/staging.tfvars` applies
staging's settings to production's state, and Terraform will not stop you.

## Resources

Two SSM parameters per environment, both free.
