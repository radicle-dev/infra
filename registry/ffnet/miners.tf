# A scalable miner deployment and the dedicated node pool of preemptible c2
# instances.
#
# The secret key for the beneficiary account for mining is stored in
# `../secrets.yaml`.

resource "kubernetes_deployment" "miner" {
  lifecycle {
    ignore_changes = [
      spec[0].replicas,
    ]
  }

  metadata {
    name = "miner"
    labels = {
      app = "miner"
    }
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

        toleration {
          key      = "preemptible"
          operator = "Exists"
        }

        # We want mining pods to only run on mining nodes and be spread
        # evenly across those nodes. To achieve this we make mining
        # pods anti affine to themselves.
        affinity {
          node_affinity {
            required_during_scheduling_ignored_during_execution {
              node_selector_term {
                match_expressions {
                  key      = "mining"
                  operator = "In"
                  values   = ["true"]
                }
              }
            }
          }

          pod_anti_affinity {
            preferred_during_scheduling_ignored_during_execution {
              weight = 100

              pod_affinity_term {
                label_selector {
                  match_expressions {
                    key      = "app"
                    operator = "In"
                    values   = ["miner"]
                  }
                }

                topology_key = "kubernetes.io/hostname"
              }
            }
          }
        }


        container {
          image   = local.node_image
          name    = "radicle-registry-node"
          command = ["radicle-registry-node"]
          args = [
            "--mine=5DtBmCrC6r31Tysk4NGZGhFyJTd6EFEx4ULPN8qrb5HAgGch",
            "--bootnodes=/dns4/validator-0.validator/tcp/30333/p2p/QmdEvLkAS8mxETQy1RCbdmcPPzxSs9RbExFcWvwJZDXxjG",
            "--bootnodes=/dns4/validator-1.validator/tcp/30333/p2p/QmceS5WYfDyKNtnzrxCw4TEL9nokvJkRi941oUzBvErsuD",
            "--chain=ffnet",
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

resource "google_container_node_pool" "mining" {
  provider   = google-beta
  cluster    = google_container_cluster.ffnet.name
  name       = "mining"
  node_count = 2

  node_config {
    preemptible  = true
    machine_type = "c2-standard-4"

    taint {
      key    = "mining"
      value  = "true"
      effect = "NO_EXECUTE"
    }

    taint {
      key    = "preemptible"
      value  = "true"
      effect = "NO_EXECUTE"
    }

    labels = {
      mining = "true"
    }
  }

  upgrade_settings {
    max_surge       = 1
    max_unavailable = 1
  }

  lifecycle {
    ignore_changes = [
      # We want to control this manually to scale mining
      node_count
    ]
  }
}
