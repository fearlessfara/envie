variable "environment" {
  description = "Environment these names belong to"
  type        = string
}

output "prefix" {
  description = "Prefix every resource name should carry"
  value       = "envie-test-monorepo-${var.environment}"
}

output "tags" {
  description = "Tags every resource should carry"
  value = {
    Project     = "envie-test-monorepo"
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}
