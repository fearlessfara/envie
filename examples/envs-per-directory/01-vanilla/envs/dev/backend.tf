terraform {
  backend "s3" {
    bucket         = "envie-test-envdirs-tfstate"
    key            = "dev/terraform.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "envie-test-envdirs-tflocks"
    encrypt        = true
  }
}
