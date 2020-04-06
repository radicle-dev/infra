# Deployment of miners.
#
# They connect to the validator boot node and do not expose any other
# APIs.
#
# The miners have a toleration for the `mining` key so that they can
# run on the mining pool.
#
# The deployment does not use persistent storage so miners will need
# to sync on startup.
resource "kubernetes_deployment" "devnet-miner" {
  lifecycle {
    ignore_changes = [
      # The image is updated by the radicle-registry CI
      spec[0].template[0].spec[0].container[0].image,
      # This may be manually updated
      spec[0].replicas,
    ]
  }

  metadata {
    name      = "miner"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }

  spec {
    replicas = 2

    selector {
      match_labels = {
        app = "miner"
      }
    }

    template {
      metadata {
        labels = {
          app = "miner"
        }

        annotations = {
          "prometheus.io/port" = "9615"
        }
      }

      spec {
        termination_grace_period_seconds = 3

        toleration {
          key      = "mining"
          operator = "Exists"
        }

        container {
          image   = "gcr.io/opensourcecoin/radicle-registry/node:3deb30f6843b2396819b74d5ad18682e3eec08c1"
          name    = "radicle-registry-node"
          command = ["radicle-registry-node"]
          args = [
            # The SS58 address for the seed string //Mine
            "--mine=5HYpUCg4KKiwpih63PUHmGeNrK2XeTxKR83yNKbZeTsvSKNq",
            "--bootnodes=/dns4/validator-0.validator.devnet.svc.cluster.local/tcp/30333/p2p/QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR",
            "--chain=devnet",
            "--prometheus-external"
          ]

          port {
            name           = "p2p"
            container_port = 30333
          }

          port {
            name           = "prometheus"
            container_port = 9615
          }

          resources {
            requests {
              cpu    = "700m"
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
