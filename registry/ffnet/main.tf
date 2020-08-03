variable "project" {
  type = string
}

locals {
  node_image = "gcr.io/opensourcecoin/radicle-registry/node:edf3943c4d4a2cc621fd0d880d2fbbf0de6a4d2e"
}

provider "google-beta" {
  version = "~>3.2"
  project = var.project
  region  = "europe-west1"
  zone    = "europe-west1-c"
}

resource "google_container_cluster" "ffnet" {
  provider = google-beta
  name     = "radicle-registry-ffnet"

  # Enable VPC native cluster
  ip_allocation_policy {}

  release_channel {
    channel = "REGULAR"
  }

  # Node pool is managed by terraform
  remove_default_node_pool = true
  initial_node_count       = 1
}

resource "google_container_node_pool" "pool-1" {
  provider   = google-beta
  cluster    = google_container_cluster.ffnet.name
  name       = "pool-1"
  node_count = 1
  node_config {
    machine_type = "n1-standard-2"
  }

  upgrade_settings {
    max_surge       = 1
    max_unavailable = 1
  }
}

data "google_client_config" "default" {}

provider "kubernetes" {
  load_config_file       = false
  token                  = data.google_client_config.default.access_token
  host                   = "https://${google_container_cluster.ffnet.endpoint}"
  cluster_ca_certificate = base64decode(google_container_cluster.ffnet.master_auth.0.cluster_ca_certificate)
}
