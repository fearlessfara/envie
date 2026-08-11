# Hand-wired to the data stack's production state key.
# Envie will override this per environment; the block can stay.
data "terraform_remote_state" "data" {
  backend = "s3"

  config = {
    bucket = "envie-test-multistack-tfstate"
    key    = "prod/data/terraform.tfstate"
    region = "eu-west-1"
  }
}
