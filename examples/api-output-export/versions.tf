terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.15.0"
    }
    archive = {
      source  = "hashicorp/archive"
      version = "~> 2.0"
    }
  }

  backend "s3" {
    bucket         = "envie-test-api-export-tfstate"
    key            = "prod/terraform.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "envie-test-api-export-tflocks"
    encrypt        = true
  }
}
