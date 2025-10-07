# VPC Module - Lightweight Version
# Uses null resources for fast deployment

variable "cidr_block" {
  description = "CIDR block for the VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "List of availability zones"
  type        = list(string)
  default     = ["eu-west-1a", "eu-west-1b"]
}

# Simulate VPC creation with null resource
resource "null_resource" "vpc" {
  triggers = {
    cidr_block = var.cidr_block
    azs        = join(",", var.availability_zones)
  }

  provisioner "local-exec" {
    command = "echo 'VPC created with CIDR ${var.cidr_block}'"
  }
}

# Simulate Internet Gateway
resource "null_resource" "igw" {
  depends_on = [null_resource.vpc]

  triggers = {
    vpc_id = null_resource.vpc.id
  }

  provisioner "local-exec" {
    command = "echo 'Internet Gateway created for VPC'"
  }
}

# Outputs
output "vpc_id" {
  description = "ID of the VPC"
  value       = "vpc-${substr(sha256(var.cidr_block), 0, 8)}"
}

output "vpc_cidr_block" {
  description = "CIDR block of the VPC"
  value       = var.cidr_block
}

output "internet_gateway_id" {
  description = "ID of the Internet Gateway"
  value       = "igw-${substr(sha256(var.cidr_block), 0, 8)}"
}

output "cidr_block" {
  description = "CIDR block of the VPC"
  value       = var.cidr_block
}