provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "envie-test-static-site"
      Environment = var.environment
      ManagedBy   = "terraform"
    }
  }
}
