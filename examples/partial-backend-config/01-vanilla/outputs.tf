output "greeting_parameter" {
  description = "Name of the greeting parameter"
  value       = aws_ssm_parameter.greeting.name
}

output "log_level" {
  description = "Log level this environment was built with"
  value       = var.log_level
}
