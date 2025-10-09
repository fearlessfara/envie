terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.15.0"
    }
  }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = local.envie_common_tags
  }
}

variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "eu-west-1"
}

# Use the official Terraform AWS DynamoDB module
module "dynamodb_table" {
  source  = "terraform-aws-modules/dynamodb-table/aws"
  version = "~> 5.1.0"

  name     = "${local.envie_project_name}-${local.envie_environment_id}-items"
  hash_key = "id"

  attributes = [
    {
      name = "id"
      type = "S"
    }
  ]

  billing_mode = "PAY_PER_REQUEST"

  # Enable point-in-time recovery for production
  point_in_time_recovery_enabled = true

  # Enable server-side encryption
  server_side_encryption_enabled     = true
  server_side_encryption_kms_key_arn = null # Use AWS managed key

  # TTL
  ttl_enabled        = true
  ttl_attribute_name = "ttl"

  tags = {
    Name = "${local.envie_project_name}-${local.envie_environment_id}-items"
  }
}

# Outputs that Lambda will use
output "table_name" {
  description = "DynamoDB table name"
  value       = module.dynamodb_table.dynamodb_table_id
}

output "table_arn" {
  description = "DynamoDB table ARN"
  value       = module.dynamodb_table.dynamodb_table_arn
}

output "table_stream_arn" {
  description = "DynamoDB table stream ARN"
  value       = module.dynamodb_table.dynamodb_table_stream_arn
}
