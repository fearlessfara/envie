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
  api_root_resource_id = data.terraform_remote_state.core.outputs.api_root_resource_id
  api_execution_arn    = data.terraform_remote_state.core.outputs.api_execution_arn
  stage_name           = data.terraform_remote_state.core.outputs.stage_name
}

# Reference outputs from db unit (DynamoDB)
# Envie generates this automatically in remote_state.envie.tf:
# data "terraform_remote_state" "db" { ... }
locals {
  table_name = data.terraform_remote_state.db.outputs.table_name
  table_arn  = data.terraform_remote_state.db.outputs.table_arn
}

# Create Lambda function using official Terraform module
module "lambda_function" {
  source  = "terraform-aws-modules/lambda/aws"
  version = "~> 8.1.0"

  function_name = "${local.envie_project_name}-${local.envie_environment_id}-items-api"
  description   = "Handle CRUD operations for items in DynamoDB"
  handler       = "index.handler"
  runtime       = "nodejs20.x"
  timeout       = 30
  memory_size   = 256

  # Source code
  source_path = "${path.module}/lambda"

  # Environment variables
  environment_variables = {
    TABLE_NAME   = local.table_name
    ENVIRONMENT  = local.envie_environment_id
    PROJECT_NAME = local.envie_project_name
  }

  # IAM role configuration
  attach_policy_statements = true
  policy_statements = {
    dynamodb = {
      effect = "Allow"
      actions = [
        "dynamodb:GetItem",
        "dynamodb:PutItem",
        "dynamodb:UpdateItem",
        "dynamodb:DeleteItem",
        "dynamodb:Query",
        "dynamodb:Scan"
      ]
      resources = [local.table_arn]
    }
  }

  # CloudWatch Logs
  cloudwatch_logs_retention_in_days = 7

  # Allow API Gateway to invoke
  allowed_triggers = {
    APIGatewayAny = {
      service    = "apigateway"
      source_arn = "${local.api_execution_arn}/*/*"
    }
  }

  tags = {
    Name = "${local.envie_project_name}-${local.envie_environment_id}-items-api"
  }
}

# Create /items resource
resource "aws_api_gateway_resource" "items" {
  rest_api_id = local.api_id
  parent_id   = local.api_root_resource_id
  path_part   = "items"
}

# Create /items/{id} resource
resource "aws_api_gateway_resource" "item" {
  rest_api_id = local.api_id
  parent_id   = aws_api_gateway_resource.items.id
  path_part   = "{id}"
}

# POST /items - Create item
resource "aws_api_gateway_method" "post_items" {
  rest_api_id   = local.api_id
  resource_id   = aws_api_gateway_resource.items.id
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "post_items" {
  rest_api_id             = local.api_id
  resource_id             = aws_api_gateway_resource.items.id
  http_method             = aws_api_gateway_method.post_items.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = module.lambda_function.lambda_function_invoke_arn
}

# GET /items - List all items
resource "aws_api_gateway_method" "get_items" {
  rest_api_id   = local.api_id
  resource_id   = aws_api_gateway_resource.items.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_items" {
  rest_api_id             = local.api_id
  resource_id             = aws_api_gateway_resource.items.id
  http_method             = aws_api_gateway_method.get_items.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = module.lambda_function.lambda_function_invoke_arn
}

# GET /items/{id} - Get specific item
resource "aws_api_gateway_method" "get_item" {
  rest_api_id   = local.api_id
  resource_id   = aws_api_gateway_resource.item.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "get_item" {
  rest_api_id             = local.api_id
  resource_id             = aws_api_gateway_resource.item.id
  http_method             = aws_api_gateway_method.get_item.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = module.lambda_function.lambda_function_invoke_arn
}

# PUT /items/{id} - Update item
resource "aws_api_gateway_method" "put_item" {
  rest_api_id   = local.api_id
  resource_id   = aws_api_gateway_resource.item.id
  http_method   = "PUT"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "put_item" {
  rest_api_id             = local.api_id
  resource_id             = aws_api_gateway_resource.item.id
  http_method             = aws_api_gateway_method.put_item.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = module.lambda_function.lambda_function_invoke_arn
}

# DELETE /items/{id} - Delete item
resource "aws_api_gateway_method" "delete_item" {
  rest_api_id   = local.api_id
  resource_id   = aws_api_gateway_resource.item.id
  http_method   = "DELETE"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "delete_item" {
  rest_api_id             = local.api_id
  resource_id             = aws_api_gateway_resource.item.id
  http_method             = aws_api_gateway_method.delete_item.http_method
  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = module.lambda_function.lambda_function_invoke_arn
}

# Create deployment after all methods are defined
resource "aws_api_gateway_deployment" "main" {
  rest_api_id = local.api_id

  triggers = {
    redeployment = sha1(jsonencode([
      aws_api_gateway_resource.items.id,
      aws_api_gateway_resource.item.id,
      aws_api_gateway_method.post_items.id,
      aws_api_gateway_method.get_items.id,
      aws_api_gateway_method.get_item.id,
      aws_api_gateway_method.put_item.id,
      aws_api_gateway_method.delete_item.id,
      aws_api_gateway_integration.post_items.id,
      aws_api_gateway_integration.get_items.id,
      aws_api_gateway_integration.get_item.id,
      aws_api_gateway_integration.put_item.id,
      aws_api_gateway_integration.delete_item.id,
    ]))
  }

  lifecycle {
    create_before_destroy = true
  }
}

# Create stage
resource "aws_api_gateway_stage" "main" {
  deployment_id = aws_api_gateway_deployment.main.id
  rest_api_id   = local.api_id
  stage_name    = local.stage_name

  xray_tracing_enabled = true
}

# Outputs
output "lambda_function_arn" {
  description = "Lambda function ARN"
  value       = module.lambda_function.lambda_function_arn
}

output "lambda_function_name" {
  description = "Lambda function name"
  value       = module.lambda_function.lambda_function_name
}

output "api_invoke_url" {
  description = "API Gateway invoke URL"
  value       = aws_api_gateway_stage.main.invoke_url
}

output "api_endpoints" {
  description = "API endpoints"
  value = {
    create_item  = "POST ${aws_api_gateway_stage.main.invoke_url}/items"
    list_items   = "GET ${aws_api_gateway_stage.main.invoke_url}/items"
    get_item     = "GET ${aws_api_gateway_stage.main.invoke_url}/items/{id}"
    update_item  = "PUT ${aws_api_gateway_stage.main.invoke_url}/items/{id}"
    delete_item  = "DELETE ${aws_api_gateway_stage.main.invoke_url}/items/{id}"
  }
}
