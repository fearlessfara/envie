variable "project_name" {
  description = "Name of the project"
  type        = string
}

variable "bucket_name" {
  description = "Name of the S3 bucket"
  type        = string
  default     = "app-storage"
}

variable "tags" {
  description = "Common tags for all resources"
  type        = map(string)
  default     = {}
}
