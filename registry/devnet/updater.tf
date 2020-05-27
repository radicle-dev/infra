# Add service account `devnet-node-updater` that can update the
# `devnet-nodes` stateful set.
resource "kubernetes_service_account" "devnet-node-updater" {
  metadata {
    name      = "devnet-node-updater"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }
}

resource "kubernetes_role" "devnet-node-updater" {
  metadata {
    name      = "devnet-node-updater"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }

  rule {
    api_groups = ["apps"]
    resources  = ["statefulsets"]
    resource_names = [
      kubernetes_stateful_set.devnet-validator.metadata[0].name,
    ]
    verbs = ["get", "watch", "list", "update", "patch"]
  }

  rule {
    api_groups = ["apps"]
    resources  = ["deployments"]
    resource_names = [
      kubernetes_deployment.devnet-miner.metadata[0].name
    ]
    verbs = ["get", "watch", "list", "update", "patch"]
  }
}

resource "kubernetes_role_binding" "devnet-node-updater" {
  metadata {
    name      = "devnet-node-updater"
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }

  role_ref {
    api_group = "rbac.authorization.k8s.io"
    kind      = "Role"
    name      = kubernetes_role.devnet-node-updater.metadata[0].name
  }

  subject {
    kind      = "ServiceAccount"
    name      = kubernetes_service_account.devnet-node-updater.metadata[0].name
    namespace = kubernetes_namespace.devnet.metadata[0].name
  }
}
