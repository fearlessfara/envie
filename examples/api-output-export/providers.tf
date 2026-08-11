provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "envie-test-api-export"
      Environment = var.environment
      ManagedBy   = "terraform"
    }
  }
}
