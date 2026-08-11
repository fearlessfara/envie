locals {
  name_prefix = "envie-test-partial-${var.environment}"
}

resource "aws_ssm_parameter" "greeting" {
  name  = "/${local.name_prefix}/greeting"
  type  = "String"
  value = "hello from ${var.environment}"
}

resource "aws_ssm_parameter" "log_level" {
  name  = "/${local.name_prefix}/log-level"
  type  = "String"
  value = var.log_level
}
