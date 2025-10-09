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

# Create REST API Gateway
resource "aws_api_gateway_rest_api" "main" {
  name        = "${local.envie_project_name}-${local.envie_environment_id}-api"
  description = "API Gateway for ${local.envie_environment_id} environment"

  endpoint_configuration {
    types = ["REGIONAL"]
  }
}

# Note: Deployment and stage will be created by the api unit
# The api unit will create the deployment after adding methods

# CloudWatch log group for API Gateway
resource "aws_cloudwatch_log_group" "api_gateway" {
  name              = "/aws/apigateway/${local.envie_project_name}-${local.envie_environment_id}"
  retention_in_days = 7
}

# Outputs that other units will use
output "api_id" {
  description = "API Gateway REST API ID"
  value       = aws_api_gateway_rest_api.main.id
}

output "api_root_resource_id" {
  description = "API Gateway root resource ID"
  value       = aws_api_gateway_rest_api.main.root_resource_id
}

output "api_execution_arn" {
  description = "API Gateway execution ARN"
  value       = aws_api_gateway_rest_api.main.execution_arn
}

output "stage_name" {
  description = "API Gateway stage name"
  value       = local.envie_environment_id
}
