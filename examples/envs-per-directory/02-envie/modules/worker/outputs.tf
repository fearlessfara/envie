output "queue_url" {
  description = "URL of the job queue"
  value       = aws_sqs_queue.jobs.url
}

output "queue_name" {
  description = "Name of the job queue"
  value       = aws_sqs_queue.jobs.name
}
