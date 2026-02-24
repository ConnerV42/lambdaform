variable "env" {
  default = "staging"
}

variable "app" {
  default = "myapp"
}

module "child" {
  source = "./modules/child"
  env    = var.env
  app    = var.app
}
