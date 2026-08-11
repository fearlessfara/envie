provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "envie-test-workspaces"
      Environment = terraform.workspace
      ManagedBy   = "terraform"
    }
  }
}
