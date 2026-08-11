terraform {
  backend "s3" {
    bucket         = "envie-test-monorepo-tfstate"
    key            = "platform/network/terraform.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "envie-test-monorepo-tflocks"
    encrypt        = true
  }
}
