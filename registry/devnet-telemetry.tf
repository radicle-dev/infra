# Declares services and deployment for `telemetry-frontend` and
# `telemetry-backend` together with an HTTP load balancer ingress for
# them. The HTTP loadbalancer is bound to a global static IP
#
# We use `./images/telemetry-backend` and `./images/telemetry-backend`
# as container images

output "devnet-telemetry-dashboard" {
  description = "Telementry dashboard URL"
  value       = "http://${google_compute_global_address.devnet_telemetry.address}"
}

resource "kubernetes_ingress" "telementry" {
  metadata {
    name      = "telemetry"
    namespace = kubernetes_namespace.devnet.metadata[0].name

    annotations = {
      "kubernetes.io/ingress.global-static-ip-name" = google_compute_global_address.devnet_telemetry.name

    }
  }

  spec {
    backend {
      service_name = "telemetry-frontend"
      service_port = "http"
    }
    rule {
      http {
        path {
          path = "/feed/"
          backend {
            service_name = "telemetry-backend"
            service_port = "http"
          }
        }
      }
    }
  }
}

resource "google_compute_global_address" "devnet_telemetry" {
  name = "devnet-telemetry"
}

resource "kubernetes_service" "telemetry-backend" {
  metadata {
    name      = "telemetry-backend"
    namespace = kubernetes_namespace.devnet.metadata[0].name

  }

  spec {
    selector = {
      app = "telemetry-backend"
    }

    type = "NodePort"

    port {
      name        = "http"
      port        = 8000
      target_port = "http"
    }
  }
}

resource "kubernetes_deployment" "telemetry-backend" {
  metadata {
    name      = "telemetry-backend"
    namespace = kubernetes_namespace.devnet.metadata[0].name

  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "telemetry-backend"
      }
    }

    template {
      metadata {
        labels = {
          app = "telemetry-backend"
        }
      }

      spec {

        container {
          image = "gcr.io/opensourcecoin/radicle-registry/telemetry-backend:v1"
          name  = "radicle-registry-telemetry-backend"

          port {
            name           = "http"
            container_port = 8000
          }

          readiness_probe {
            http_get {
              path = "/network_state/foo/0"
              port = "http"
            }

            period_seconds        = 3
            initial_delay_seconds = 1
          }

          liveness_probe {
            http_get {
              path = "/network_state/foo/1"
              port = "http"
            }

            period_seconds        = 3
            initial_delay_seconds = 1
          }
        }
      }
    }
  }
}

resource "kubernetes_service" "telemetry-frontend" {
  metadata {
    name      = "telemetry-frontend"
    namespace = "devnet"
  }

  spec {
    selector = {
      app = "telemetry-frontend"
    }

    type = "NodePort"

    port {
      name        = "http"
      port        = 8000
      target_port = "http"
    }
  }
}

resource "kubernetes_deployment" "telemetry-frontend" {
  metadata {
    name      = "telemetry-frontend"
    namespace = kubernetes_namespace.devnet.metadata[0].name

  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "telemetry-frontend"
      }
    }

    template {
      metadata {
        labels = {
          app = "telemetry-frontend"
        }
      }

      spec {
        container {
          image = "gcr.io/opensourcecoin/radicle-registry/telemetry-frontend:v6"
          name  = "radicle-registry-telemetry-frontend"

          port {
            name           = "http"
            container_port = 8000
          }

          readiness_probe {
            http_get {
              path = "/"
              port = "http"
            }

            period_seconds        = 3
            initial_delay_seconds = 1
          }
        }
      }
    }
  }
}
