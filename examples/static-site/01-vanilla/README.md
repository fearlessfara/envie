# Static site (vanilla Terraform)

A flat single-root module: one S3 website bucket, one remote state key, `var.environment` for naming.

```bash
aws-vault exec personal --no-session -- terraform init
aws-vault exec personal --no-session -- terraform plan -var='environment=prod'

# Tear down if you applied
aws-vault exec personal --no-session -- terraform destroy -var='environment=prod'
```

Validate without a state bucket:

```bash
terraform init -backend=false
terraform validate
```
