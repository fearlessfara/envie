variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "eu-west-1"
}

variable "environment" {
  description = "Environment name used in resource names (dev, staging, prod, ...)"
  type        = string
  default     = "prod"
}
