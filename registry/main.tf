locals {
  project = "radicle-registry-dev"
}

module "devnet" {
  source = "./devnet"
}

module "ffnet" {
  source  = "./ffnet"
  project = local.project
  dns = {
    domain       = google_dns_managed_zone.radicle-network.dns_name
    managed_zone = google_dns_managed_zone.radicle-network.name
  }
}

output "ffnet-boot-node-addresses" {
  value = module.ffnet.boot-node-addresses
}

output "ffnet-node-rpc-url" {
  value = module.ffnet.node-rpc-url
}

provider "google" {
  version = "~>3.2"
  project = local.project
  region  = "europe-west1"
  zone    = "europe-west1-b"
}

provider "google-beta" {
  version = "~>3.2"
  project = local.project
  region  = "europe-west1"
  zone    = "europe-west1-b"
}
