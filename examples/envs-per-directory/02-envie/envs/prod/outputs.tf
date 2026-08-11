output "queue_url" {
  description = "URL of this environment's job queue"
  value       = module.worker.queue_url
}

output "queue_name" {
  description = "Name of this environment's job queue"
  value       = module.worker.queue_name
}
