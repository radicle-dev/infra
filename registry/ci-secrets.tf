# KMS key for managing CI secret in the `radicle-registry` repository.
#
# Creates key `ci-secrets` in keyring `dev` and grants decrypting
# permissions to the buildkite agent service account.
#
# Also grant encrypter role to developers.

resource "google_kms_key_ring" "dev" {
  name     = "dev"
  location = "global"
}


resource "google_kms_crypto_key" "ci-secrets" {
  name     = "ci-secrets"
  key_ring = google_kms_key_ring.dev.self_link

  purpose = "ENCRYPT_DECRYPT"

  lifecycle {
    prevent_destroy = true
  }
}

resource "google_kms_crypto_key_iam_binding" "ci-secrets-encrypter" {
  crypto_key_id = google_kms_crypto_key.ci-secrets.id

  role = "roles/cloudkms.cryptoKeyEncrypterDecrypter"

  members = [
    "user:thomas@monadic.xyz",
  ]
}

resource "google_kms_crypto_key_iam_binding" "ci-secrets-decrypter" {
  crypto_key_id = google_kms_crypto_key.ci-secrets.id

  role = "roles/cloudkms.cryptoKeyDecrypter"

  members = [
    "serviceAccount:buildkite-agent@opensourcecoin.iam.gserviceaccount.com",
  ]
}
