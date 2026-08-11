output "vpc_id" {
  description = "Identifier other stacks build on"
  value       = local.vpc_id
}

output "name_prefix" {
  description = "Prefix other stacks should reuse"
  value       = module.naming.prefix
}
