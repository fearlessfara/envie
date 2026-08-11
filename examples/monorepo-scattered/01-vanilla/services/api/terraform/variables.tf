// This stack calls the variable "env" while the network stack calls it
// "environment". Envie recognises both, per unit.
variable "env" {
  description = "Environment this service belongs to"
  type        = string
}
