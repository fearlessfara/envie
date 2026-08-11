# One state object, isolated per environment by Terraform workspaces.
# State for workspace "dev" lives at s3://.../env/dev/terraform.tfstate
terraform {
  backend "s3" {
    bucket               = "envie-test-workspaces-tfstate"
    key                  = "terraform.tfstate"
    region               = "eu-west-1"
    dynamodb_table       = "envie-test-workspaces-tflocks"
    encrypt              = true
    workspace_key_prefix = "env"
  }
}
