# Workspaces multi-env (vanilla Terraform)

One root module. `terraform.workspace` drives names and tags. The S3 backend uses `workspace_key_prefix` so `dev` and `staging` get separate state objects.

```bash
aws-vault exec personal --no-session -- terraform init
aws-vault exec personal --no-session -- terraform workspace new dev
aws-vault exec personal --no-session -- terraform plan
aws-vault exec personal --no-session -- terraform workspace new staging
aws-vault exec personal --no-session -- terraform plan

# Tear down the workspace you applied
aws-vault exec personal --no-session -- terraform workspace select dev
aws-vault exec personal --no-session -- terraform destroy
```

Validate without a state bucket:

```bash
terraform init -backend=false
terraform validate
```
