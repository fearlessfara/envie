provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "envie-test-multistack"
      Environment = var.environment
      Stack       = "data"
      ManagedBy   = "terraform"
    }
  }
}

resource "aws_dynamodb_table" "items" {
  name         = "envie-test-multistack-${var.environment}-items"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }

  ttl {
    enabled        = true
    attribute_name = "ttl"
  }

  point_in_time_recovery {
    enabled = true
  }
}

output "table_name" {
  description = "DynamoDB table name"
  value       = aws_dynamodb_table.items.name
}

output "table_arn" {
  description = "DynamoDB table ARN"
  value       = aws_dynamodb_table.items.arn
}
