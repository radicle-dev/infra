locals {
  node_image = "gcr.io/opensourcecoin/radicle-registry/node:c8564b7c9ab1db9563dc138bd31e7b1db0bf2ed1 "
}

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

data "google_client_config" "default" {}

provider "kubernetes" {
  load_config_file       = false
  token                  = data.google_client_config.default.access_token
  host                   = "https://${google_container_cluster.radicle-registry-devnet.endpoint}"
  cluster_ca_certificate = base64decode(google_container_cluster.radicle-registry-devnet.master_auth.0.cluster_ca_certificate)
}
