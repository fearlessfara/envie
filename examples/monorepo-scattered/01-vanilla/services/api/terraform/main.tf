data "terraform_remote_state" "network" {
  backend = "s3"

  config = {
    bucket = "envie-test-monorepo-tfstate"
    key    = "platform/network/terraform.tfstate"
    region = "eu-west-1"
  }
}

module "naming" {
  source      = "../../../modules/naming"
  environment = var.env
}

locals {
  endpoint = "https://${var.env}.api.example.internal"
}

resource "aws_ssm_parameter" "api_vpc" {
  name  = "/${module.naming.prefix}/api/vpc-id"
  type  = "String"
  value = data.terraform_remote_state.network.outputs.vpc_id
  tags  = module.naming.tags
}

resource "aws_ssm_parameter" "api_endpoint" {
  name  = "/${module.naming.prefix}/api/endpoint"
  type  = "String"
  value = local.endpoint
  tags  = module.naming.tags
}
