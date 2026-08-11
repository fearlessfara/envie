# Typical flat-repo backend: one bucket, one key for the long-lived environment.
terraform {
  backend "s3" {
    bucket         = "envie-test-static-site-tfstate"
    key            = "prod/terraform.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "envie-test-static-site-tflocks"
    encrypt        = true
  }
}
