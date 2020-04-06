# KMS key for encrypting secrets in the `registry` folder in the
# `radicle-dev/infra` repo.
#
# Access to this key is required to update our infrastructure.
#
# All maintainers of the Radicle Registry infrastructure should have
# access to this key.
resource "google_kms_crypto_key" "radicle-infra-repo" {
  name     = "radicle-infra-repo"
  key_ring = google_kms_key_ring.dev.self_link

  purpose = "ENCRYPT_DECRYPT"

  lifecycle {
    prevent_destroy = true
  }
}

resource "google_kms_crypto_key_iam_binding" "radicle-infra-repo" {
  crypto_key_id = google_kms_crypto_key.radicle-infra-repo.self_link

  role = "roles/cloudkms.cryptoKeyEncrypterDecrypter"

  members = [
    "user:igor@monadic.xyz",
    "user:kim@monadic.xyz",
    "user:nuno@monadic.xyz",
    "user:thomas@monadic.xyz",
  ]
}
