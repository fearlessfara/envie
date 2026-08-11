output "endpoint" {
  description = "Where this environment's API answers"
  value       = local.endpoint
}

output "vpc_id" {
  description = "Network this service was wired to"
  value       = data.terraform_remote_state.network.outputs.vpc_id
}
