# Expose P2P endpoints of the validator nodes as boot nodes. See the output for
# the boot node addresses.


output "boot-node-addresses" {
  value = [
    "/dns4/${google_dns_record_set.boot[0].name}/tcp/30333/p2p/QmdEvLkAS8mxETQy1RCbdmcPPzxSs9RbExFcWvwJZDXxjG",
    "/dns4/${google_dns_record_set.boot[1].name}/tcp/30333/p2p/QmceS5WYfDyKNtnzrxCw4TEL9nokvJkRi941oUzBvErsuD"
  ]
}

resource "kubernetes_service" "boot" {
  for_each = toset([
    "0", "1"
  ])

  metadata {
    name = "boot-${each.key}"
  }

  spec {
    type             = "LoadBalancer"
    load_balancer_ip = google_compute_address.ffnet-boot[each.key].address

    selector = {
      app                                  = "validator"
      "statefulset.kubernetes.io/pod-name" = "validator-${each.key}"
    }

    port {
      name        = "p2p"
      port        = 30333
      target_port = "p2p"
    }
  }
}

resource "google_compute_address" "ffnet-boot" {
  for_each = toset([
    "0", "1"
  ])
  name = "ffnet-boot-${each.key}"
}

variable "dns" {
  type = object({
    managed_zone = string
    domain       = string
  })
}

resource "google_dns_record_set" "boot" {
  for_each = toset([
    "0", "1"
  ])
  name         = "boot-${each.key}.ff.${var.dns.domain}"
  managed_zone = var.dns.managed_zone

  type = "A"
  ttl  = 600

  rrdatas = [google_compute_address.ffnet-boot[each.key].address]
}


