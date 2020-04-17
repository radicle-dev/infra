locals {
  project = "radicle-registry-dev"
}

module "devnet" {
  source = "./devnet"
}

module "ffnet" {
  source  = "./ffnet"
  project = local.project
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
