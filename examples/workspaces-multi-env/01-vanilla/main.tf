locals {
  env = terraform.workspace
}

resource "aws_sqs_queue" "dlq" {
  name                      = "envie-test-jobs-${local.env}-dlq"
  message_retention_seconds = 86400
}

resource "aws_sqs_queue" "jobs" {
  name                      = "envie-test-jobs-${local.env}"
  message_retention_seconds = 345600
  receive_wait_time_seconds = 20

  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.dlq.arn
    maxReceiveCount     = 3
  })
}
