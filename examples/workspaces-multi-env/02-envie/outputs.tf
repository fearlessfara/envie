output "queue_url" {
  description = "Main jobs queue URL"
  value       = aws_sqs_queue.jobs.url
}

output "dlq_url" {
  description = "Dead-letter queue URL"
  value       = aws_sqs_queue.dlq.url
}

output "environment" {
  description = "Terraform workspace this state belongs to"
  value       = terraform.workspace
}
