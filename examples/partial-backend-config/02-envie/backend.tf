terraform {
  # Deliberately empty: the bucket and key differ per environment and are
  # supplied at init time with
  #   terraform init -backend-config=config/prod.s3.tfbackend
  backend "s3" {}
}
