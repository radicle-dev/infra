# Statefulset of two validator nodes.

# Headless service to be able to address the nodes with DNS.
resource "kubernetes_service" "validator" {
  metadata {
    name = "validator"
  }

  spec {
    type       = "ClusterIP"
    cluster_ip = "None"

    selector = {
      app = "validator"
    }
  }
}


resource "kubernetes_stateful_set" "validator" {
  lifecycle {
    ignore_changes = [spec[0].template[0].spec[0].container[0].image]
  }

  metadata {
    name = "validator"
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

        subdomain = "foo"

        container {
          image   = local.node_image
          name    = "radicle-registry-node"
          command = ["bash", "-c"]
          args = [<<SCRIPT
            set -euo pipefail

            pod_index=$(echo "$KUBERNETES_POD_NAME" | sed -E 's/^.*([0-9]+)$/\1/')
            node_key_file="/var/run/secrets/validator-keys/validator-$pod_index"
            base64 -d "$node_key_file" > /tmp/node-key

            exec radicle-registry-node \
              --chain=ffnet \
              --prometheus-external \
              --node-key-file /tmp/node-key
            SCRIPT
          ]

          env {
            name = "KUBERNETES_POD_NAME"
            value_from {
              field_ref { field_path = "metadata.name" }
            }
          }

          port {
            name           = "p2p"
            container_port = 30333
          }

          port {
            name           = "prometheus"
            container_port = 9615
          }

          volume_mount {
            name       = "validator-chain-data"
            mount_path = "/data"
          }

          resources {
            requests {
              cpu    = "200m"
              memory = "300Mi"
            }
          }

          volume_mount {
            name       = "validator-keys"
            mount_path = "/var/run/secrets/validator-keys"
          }
        }

        volume {
          name = "validator-keys"
          secret {
            secret_name = kubernetes_secret.validator-keys.metadata[0].name
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

resource "kubernetes_secret" "validator-keys" {
  metadata {
    name = "validator-keys"
    labels = {
      app = "validator"
    }
  }

  data = data.external.sops-ffnet_validator_node_keys.result
}

data "external" "sops-ffnet_validator_node_keys" {
  program = [
    "sops",
    "--decrypt",
    "--output-type=json",
    "--extract=[\"ffnet_validator_node_keys\"]",
    "${path.root}/secrets.yaml",
  ]
}
