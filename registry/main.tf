locals {
  project = "opensourcecoin"
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
