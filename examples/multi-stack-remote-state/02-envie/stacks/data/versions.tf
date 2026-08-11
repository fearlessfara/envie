terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.15.0"
    }
  }

  backend "s3" {
    bucket         = "envie-test-multistack-tfstate"
    key            = "prod/data/terraform.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "envie-test-multistack-tflocks"
    encrypt        = true
  }
}
