variable "project_name" {
  description = "Name of the project"
  type        = string
}

variable "table_name" {
  description = "Name of the DynamoDB table"
  type        = string
  default     = "demo-table"
}

variable "tags" {
  description = "Common tags for all resources"
  type        = map(string)
  default     = {}
}
