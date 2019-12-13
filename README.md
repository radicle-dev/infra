# ci
CI infrastructure

## Caching

Each job container has a cache volume mounted at `/cache`. The cache volume is
shared between jobs for the same branch on the same runner. This means that jobs
on different runners or branches cannot share the cache.

For branch builds the cache volume is created from a snapshot of the cache
volume of the master branch of the runner.

The cache volume has a quota of 8GiB. This value can be configured through
`CACHE_QUOTA_GiB` in `./linux/etc/buildkite-agent/hooks/command`.

## Secrets

The build agent probes for a file `.buildkite/secrets.yaml` in the source
checkout, and if it exists, attempts to decrypt it using [`sops`][sops] in
"dotenv" format into a file `.secrets` at the root of the source checkout.

Repositories making using of this feature must:

1. Create a new symmetric key in the GCP KMS.
2. Grant the `cloudkms.cryptoKeyEncrypterDecrypter` IAM role to all contributors
   who should be able to view / modify the secrets.
3. Grant the `cloudkms.cryptoKeyDecrypter` IAM role to the `buildkite-agent`
   service account.
4. Create a `.sops.yaml` file at the root of the repository, which specifies the
   GCP KMS key to use for encrypting / decrypting the `.buildkite/secrets.yaml`
   file. See [sops documentation](https://github.com/mozilla/sops#using-sops-yaml-conf-to-select-kms-pgp-for-new-files)
   for details.

[sops]: https://github.com/mozilla/sops
