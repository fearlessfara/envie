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

# Reference outputs from core unit (API Gateway)
# Envie generates this automatically in remote_state.envie.tf:
# data "terraform_remote_state" "core" { ... }
locals {
  api_id               = data.terraform_remote_state.core.outputs.api_id
  api_execution_arn    = data.terraform_remote_state.core.outputs.api_execution_arn
  api_invoke_url       = data.terraform_remote_state.core.outputs.stage_name != null ? "https://${data.terraform_remote_state.core.outputs.api_id}.execute-api.${var.aws_region}.amazonaws.com/${data.terraform_remote_state.core.outputs.stage_name}" : null
}

# Reference outputs from api unit (Lambda)
# Envie generates this automatically in remote_state.envie.tf:
# data "terraform_remote_state" "api" { ... }
locals {
  lambda_function_arn = data.terraform_remote_state.api.outputs.lambda_function_arn
  lambda_function_name = data.terraform_remote_state.api.outputs.lambda_function_name
}

# Simple Step Functions state machine that does nothing but demonstrates dependency usage
resource "aws_sfn_state_machine" "workflow" {
  name     = "${local.envie_project_name}-${local.envie_environment_id}-workflow"
  role_arn = aws_iam_role.stepfunctions.arn

  definition = jsonencode({
    Comment = "Workflow that references API Gateway and Lambda outputs"
    StartAt = "Pass"
    States = {
      Pass = {
        Type = "Pass"
        Parameters = {
          "api_id" = local.api_id
          "lambda_arn" = local.lambda_function_arn
          "api_url" = local.api_invoke_url
        }
        End = true
      }
    }
  })

  tags = {
    Name = "${local.envie_project_name}-${local.envie_environment_id}-workflow"
  }
}

# IAM role for Step Functions
resource "aws_iam_role" "stepfunctions" {
  name = "${local.envie_project_name}-${local.envie_environment_id}-stepfunctions-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "states.amazonaws.com"
        }
      }
    ]
  })

  tags = {
    Name = "${local.envie_project_name}-${local.envie_environment_id}-stepfunctions-role"
  }
}

# Allow Step Functions to invoke Lambda
resource "aws_iam_role_policy" "stepfunctions_lambda" {
  name = "${local.envie_project_name}-${local.envie_environment_id}-stepfunctions-lambda"
  role = aws_iam_role.stepfunctions.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "lambda:InvokeFunction"
        ]
        Resource = local.lambda_function_arn
      }
    ]
  })
}

# Outputs
output "state_machine_arn" {
  description = "Step Functions state machine ARN"
  value       = aws_sfn_state_machine.workflow.arn
}

output "state_machine_name" {
  description = "Step Functions state machine name"
  value       = aws_sfn_state_machine.workflow.name
}

output "referenced_outputs" {
  description = "Outputs from dependencies that this unit references"
  value = {
    api_id          = local.api_id
    api_execution_arn = local.api_execution_arn
    api_invoke_url  = local.api_invoke_url
    lambda_arn     = local.lambda_function_arn
    lambda_name     = local.lambda_function_name
  }
}

