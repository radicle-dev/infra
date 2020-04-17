# This domain is provided by Namecheap
resource "google_dns_managed_zone" "radicle-network" {
  name     = "radicle-network"
  dns_name = "radicle.network."
}

output "radicle-network-nameservers" {
  value       = google_dns_managed_zone.radicle-network.name_servers
  description = "Name server to set on the radicle.network domain on Namecheap"
}
