module "naming" {
  source      = "../../modules/naming"
  environment = var.environment
}

locals {
  # Stands in for a VPC: the point of this example is the layout, and a real VPC
  # would cost money to leave lying around.
  vpc_id = "vpc-${substr(sha1(module.naming.prefix), 0, 12)}"
}

resource "aws_ssm_parameter" "vpc_id" {
  name  = "/${module.naming.prefix}/network/vpc-id"
  type  = "String"
  value = local.vpc_id
  tags  = module.naming.tags
}

resource "aws_ssm_parameter" "subnet_ids" {
  name  = "/${module.naming.prefix}/network/subnet-ids"
  type  = "StringList"
  value = "subnet-a,subnet-b"
  tags  = module.naming.tags
}
