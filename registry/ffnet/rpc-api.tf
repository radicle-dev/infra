# Public Node RPC service.
#
# Consists of a deployment, a load balancer and a DNS record for the
# loadbalancer IP.

output "node-rpc-url" {
  description = "RPC API URL"
  value       = "ws://${google_dns_record_set.node-rpc.name}:9944"
}

resource "kubernetes_service" "node-rpc" {
  metadata {
    name = "node-rpc"
  }

  spec {
    type             = "LoadBalancer"
    load_balancer_ip = google_compute_address.node-rpc.address

    selector = kubernetes_deployment.rpc-server.metadata[0].labels

    port {
      name        = "ws-rpc"
      port        = 9944
      target_port = "ws-rpc"
    }
  }
}

resource "kubernetes_deployment" "rpc-server" {
  lifecycle {
    ignore_changes = [
      spec[0].template[0].spec[0].container[0].image,
    ]
  }

  metadata {
    name = "rpc-server"
    labels = {
      app = "rpc-server"
    }
  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "rpc-server"
      }
    }

    template {
      metadata {
        labels = {
          app = "rpc-server"
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
          command = ["radicle-registry-node"]
          args = [
            "--bootnodes=/dns4/validator-0.validator/tcp/30333/p2p/QmdEvLkAS8mxETQy1RCbdmcPPzxSs9RbExFcWvwJZDXxjG",
            "--bootnodes=/dns4/validator-1.validator/tcp/30333/p2p/QmceS5WYfDyKNtnzrxCw4TEL9nokvJkRi941oUzBvErsuD",
            "--chain=ffnet",
            "--prometheus-external",
            "--unsafe-rpc-external"
          ]

          port {
            name           = "ws-rpc"
            container_port = 9944
          }

          port {
            name           = "prometheus"
            container_port = 9615
          }

          resources {
            requests {
              cpu    = "200m"
              memory = "300Mi"
            }

            limits {
              cpu    = "200m"
              memory = "300Mi"
            }
          }

          volume_mount {
            name       = "chain-data"
            mount_path = "/data"
          }
        }

        volume {
          name = "chain-data"
          empty_dir {}
        }
      }
    }
  }
}

resource "google_compute_address" "node-rpc" {
  name = "node-rpc"
}

resource "google_dns_record_set" "node-rpc" {
  name         = "rpc.ff.${var.dns.domain}"
  managed_zone = var.dns.managed_zone

  type = "A"
  ttl  = 600

  rrdatas = [google_compute_address.node-rpc.address]
}
