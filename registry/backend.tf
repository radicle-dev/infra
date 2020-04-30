resource "google_storage_bucket" "terraform-state" {
  name               = "radicle-registry-dev--terraform-state"
  location           = "EU"
  bucket_policy_only = true

  versioning {
    enabled = true
  }
}

terraform {
  backend "gcs" {
    bucket = "radicle-registry-dev--terraform-state"
  }
}
