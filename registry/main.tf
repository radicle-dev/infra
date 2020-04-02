locals {
  project = "radicle-registry-dev"
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

data "google_client_config" "default" {}

resource "google_container_cluster" "radicle-registry-devnet" {
  provider = google-beta
  name     = "radicle-registry-devnet"

  # Enable VPC native cluster
  ip_allocation_policy {}

  release_channel {
    channel = "REGULAR"
  }

  # Node pool is managed by terraform
  remove_default_node_pool = true
  initial_node_count       = 1
}

resource "google_container_node_pool" "radicle-registry-devnet--pool-1" {
  provider   = google-beta
  cluster    = google_container_cluster.radicle-registry-devnet.name
  name       = "pool-1"
  node_count = 2
  node_config {
    machine_type = "n1-standard-1"
  }
}

resource "google_container_node_pool" "radicle-registry-devnet--mining" {
  provider   = google-beta
  cluster    = google_container_cluster.radicle-registry-devnet.name
  name       = "mining"
  node_count = 2
  node_config {
    preemptible  = true
    machine_type = "n2-highcpu-2"

    taint {
      key    = "mining"
      value  = "true"
      effect = "NO_EXECUTE"
    }
  }
}

provider "kubernetes" {
  load_config_file       = false
  token                  = data.google_client_config.default.access_token
  host                   = "https://${google_container_cluster.radicle-registry-devnet.endpoint}"
  cluster_ca_certificate = base64decode(google_container_cluster.radicle-registry-devnet.master_auth.0.cluster_ca_certificate)
}
