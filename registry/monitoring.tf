# Defines the `monitoring` namespace with a Prometheus stateful set.
#
# The prometheus configuration can be found at `./prometheus.yaml`.
#
# We’re only monitoring pods with the `prometheus.io/port` annotation.
# If this annotation is set, the port is used to scrape the metrics.
#
resource "kubernetes_namespace" "monitoring" {
  metadata {
    name = "monitoring"
  }
}

resource "kubernetes_service" "prometheus" {
  metadata {
    name      = "prometheus"
    namespace = kubernetes_namespace.monitoring.metadata[0].name
  }

  spec {
    type = "ClusterIP"

    selector = {
      app = "prometheus"
    }

    port {
      name        = "api"
      port        = "9090"
      target_port = "api"
    }
  }
}

resource "kubernetes_stateful_set" "prometheus" {
  metadata {
    name      = "prometheus"
    namespace = kubernetes_namespace.monitoring.metadata[0].name
  }

  spec {
    replicas = 1

    service_name = "prometheus"

    selector {
      match_labels = {
        app = "prometheus"
      }
    }

    update_strategy {
      type = "RollingUpdate"
    }

    template {
      metadata {
        labels = {
          app = "prometheus"
        }
      }

      spec {
        service_account_name            = kubernetes_service_account.prometheus.metadata[0].name
        automount_service_account_token = true

        init_container {
          name    = "set-storage-permissions"
          image   = "busybox:1.31.1"
          command = ["chown", "nobody:nogroup", "/prometheus"]

          volume_mount {
            name       = "prometheus-data"
            mount_path = "/prometheus"
          }
        }

        container {
          image = "prom/prometheus:v2.17.1"
          name  = "prometheus"
          args = [
            "--config.file=/etc/prometheus/prometheus.yml",
            "--storage.tsdb.wal-compression",
            "--storage.tsdb.path=/prometheus",
            "--web.console.libraries=/usr/share/prometheus/console_libraries",
            "--web.console.templates=/usr/share/prometheus/consoles",
            "--web.enable-lifecycle"
          ]

          port {
            name           = "api"
            container_port = 9090
          }

          resources {
            requests {
              cpu    = "80m"
              memory = "50Mi"
            }
          }

          readiness_probe {
            http_get {
              path = "/-/ready"
              port = 9090
            }

            initial_delay_seconds = 30
            period_seconds        = 10
            failure_threshold     = 12
          }

          liveness_probe {
            http_get {
              path = "/-/healthy"
              port = 9090
            }

            initial_delay_seconds = 60
          }

          volume_mount {
            name       = "prometheus-data"
            mount_path = "/prometheus"
          }

          volume_mount {
            name       = "prometheus-config"
            mount_path = "/etc/prometheus"
          }
        }

        volume {
          name = "prometheus-config"
          config_map {
            name = "prometheus"
          }
        }
      }
    }


    volume_claim_template {
      metadata {
        name = "prometheus-data"
      }

      spec {
        access_modes       = ["ReadWriteOnce"]
        storage_class_name = "standard"

        resources {
          requests = {
            storage = "16Gi"
          }
        }
      }
    }
  }
}

resource "kubernetes_config_map" "prometheus" {
  metadata {
    name      = "prometheus"
    namespace = kubernetes_namespace.monitoring.metadata[0].name
  }

  data = {
    "prometheus.yml" = file("prometheus.yaml")
  }
}

resource "kubernetes_service_account" "prometheus" {
  metadata {
    name      = "prometheus"
    namespace = kubernetes_namespace.monitoring.metadata[0].name
  }
}

resource "kubernetes_cluster_role" "prometheus" {
  metadata {
    name = "prometheus"
  }

  rule {
    api_groups = [""]
    resources  = ["pods"]
    verbs      = ["get", "list", "watch"]
  }
}

resource "kubernetes_cluster_role_binding" "prometheus" {
  metadata {
    name = "prometheus"
  }

  role_ref {
    api_group = "rbac.authorization.k8s.io"
    kind      = "ClusterRole"
    name      = kubernetes_cluster_role.prometheus.metadata[0].name
  }

  subject {
    kind      = "ServiceAccount"
    name      = kubernetes_service_account.prometheus.metadata[0].name
    namespace = kubernetes_service_account.prometheus.metadata[0].namespace
  }
}
