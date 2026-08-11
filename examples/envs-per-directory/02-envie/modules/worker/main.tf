resource "aws_sqs_queue" "jobs" {
  name                      = "${var.name_prefix}-jobs"
  message_retention_seconds = 3600
}

resource "aws_ssm_parameter" "queue_url" {
  name  = "/${var.name_prefix}/queue-url"
  type  = "String"
  value = aws_sqs_queue.jobs.url
}
