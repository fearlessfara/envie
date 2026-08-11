module "worker" {
  source = "../../modules/worker"

  name_prefix = "envie-test-envdirs-${var.environment}"
}
