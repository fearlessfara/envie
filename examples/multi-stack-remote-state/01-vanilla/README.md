# Multi-stack remote state (vanilla Terraform)

Two roots, applied in order. `stacks/api` reads `stacks/data` through a hand-written `data.terraform_remote_state`.

```bash
# 1. Data stack (DynamoDB)
cd stacks/data
aws-vault exec personal --no-session -- terraform init
aws-vault exec personal --no-session -- terraform plan -var='environment=prod'

# 2. API stack (Lambda + HTTP API) — after data has been applied
cd ../api
aws-vault exec personal --no-session -- terraform init
aws-vault exec personal --no-session -- terraform plan -var='environment=prod'

# Tear down in reverse order
aws-vault exec personal --no-session -- terraform destroy -var='environment=prod'
cd ../data
aws-vault exec personal --no-session -- terraform destroy -var='environment=prod'
```

Validate without a state bucket:

```bash
cd stacks/data && terraform init -backend=false && terraform validate
cd ../api && terraform init -backend=false && terraform validate
```

`stacks/api` plan will fail until the data stack has state at `prod/data/terraform.tfstate`.
