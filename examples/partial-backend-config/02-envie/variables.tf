variable "environment" {
  description = "Environment name, supplied by the matching tfvars file"
  type        = string
}

variable "log_level" {
  description = "Application log level, which differs per environment"
  type        = string
  default     = "info"
}
