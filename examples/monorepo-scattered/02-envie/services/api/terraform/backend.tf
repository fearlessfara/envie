terraform {
  backend "s3" {
    bucket         = "envie-test-monorepo-tfstate"
    key            = "services/api/terraform.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "envie-test-monorepo-tflocks"
    encrypt        = true
  }
}
