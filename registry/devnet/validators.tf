# Declares a stateful set of two validator nodes with persistent
# volumes that run the registry devnet chain without mining.
#
# The Websocket RPC APIs of these nodes are exposed via the
# `devnet-rpc-api` loadbalancer.
#
# The first node’s P2P port is exposed via the `devnet-p2p`
# loadbalancer to serve as a boot node.
#
# We are using the node docker image built by the `radicle-registry` CI.

resource "kubernetes_namespace" "devnet" {
  metadata {
    name = "devnet"
  }
}

# Expose Websocket RPC APIs of all nodes
resource "kubernetes_service" "devnet-rpc-api" {
  metadata {
    name      = "validator-rpc-api"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }

  spec {
    type             = "LoadBalancer"
    load_balancer_ip = google_compute_address.devnet-rpc.address

    selector = {
      app = "validator"
    }

    port {
      name        = "ws-rpc-api"
      port        = 9944
      target_port = "ws-rpc-api"
    }
  }
}

resource "google_compute_address" "devnet-rpc" {
  name = "devnet-rpc"
}

output "devnet-rpc-ip" {
  description = "RPC API URL"
  value       = "ws://${google_compute_address.devnet-rpc.address}:9944"
}

# Expose P2P port of first devent node
resource "kubernetes_service" "devnet-validator-p2p" {
  metadata {
    name      = "validator-p2p-0"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }

  spec {
    type             = "LoadBalancer"
    load_balancer_ip = google_compute_address.devnet-p2p-0.address

    selector = {
      app                                  = "validator"
      "statefulset.kubernetes.io/pod-name" = "validator-0"
    }

    port {
      name        = "p2p"
      port        = 30333
      target_port = "p2p"
    }
  }
}

resource "google_compute_address" "devnet-p2p-0" {
  name = "devnet-p2p-0"
}

output "devnet-p2p-0-ip" {
  description = "P2P multiaddr for connecting to the first node"
  value       = "/ip4/${google_compute_address.devnet-p2p-0.address}/tcp/30333/p2p/QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR"
}


# Headless service to be able to address the nodes with DNS.
resource "kubernetes_service" "devnet-validator" {
  metadata {
    name      = "validator"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }

  spec {
    selector = {
      app = "validator"
    }

    cluster_ip = "None"
  }
}


resource "kubernetes_stateful_set" "devnet-validator" {
  lifecycle {
    ignore_changes = [spec[0].template[0].spec[0].container[0].image]
  }

  metadata {
    name      = "validator"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }

  spec {
    replicas = 2

    service_name = "validator"

    selector {
      match_labels = {
        app = "validator"
      }
    }

    update_strategy {
      type = "RollingUpdate"
    }

    template {
      metadata {
        labels = {
          app = "validator"
        }

        annotations = {
          "prometheus.io/port" = "9615"
        }
      }

      spec {
        termination_grace_period_seconds = 3

        container {
          image   = local.node_image
          name    = "radicle-registry-node"
          command = ["bash", "-c"]
          args    = [file("${path.module}/run-validator.sh")]

          port {
            name           = "ws-rpc-api"
            container_port = 9944
          }

          port {
            name           = "p2p"
            container_port = 30333
          }

          port {
            name           = "prometheus"
            container_port = 9615
          }


          env {
            name = "KUBERNETES_POD_NAME"
            value_from {
              field_ref { field_path = "metadata.name" }
            }
          }

          resources {
            requests {
              cpu    = "200m"
              memory = "300Mi"
            }
          }

          volume_mount {
            name       = "validator-chain-data"
            mount_path = "/data"
          }
        }
      }
    }

    volume_claim_template {
      metadata {
        name = "validator-chain-data"
      }

      spec {
        access_modes       = ["ReadWriteOnce"]
        storage_class_name = "standard"
        resources {
          requests = {
            storage = "4Gi"
          }
        }
      }
    }
  }
}
