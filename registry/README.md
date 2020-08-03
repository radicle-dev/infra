# Radicle Registry Infrastructure

This repository contains Terraform code that describes and manages the
infrastructure maintained by the Radicle Registry project.


## Managed Infrastructure

* Google Cloud Computing project `radicle-registry-dev`
* KMS key for managing
  * CI secrets in the [`radicle-registry`][radicle-registry] repository. See `./kms-ci.tf`.
  * Secrets in this repository. See `./kms-infra.tf`
* GKE cluster `radicle-registry-ffnet` for running the FFnet information. See `./ffnet`.
* DNS zones in `./dns.tf`
* Currently not provisioned: GKE cluster `radicle-registry-devnet` for running a
  devnet that we can play around with. See `./devnet`.

Run `terraform output` for information about entry points.

[radicle-registry]: https://github.com/radicle-dev/radicle-registry


## Monitoring

We use [Grafana Cloud][grafana-cloud] to monitor the Registry nodes. You can
find the Grafana instance at [`radicle.grafana.net`][radicle-grafaa]

We monitor the underlying infrastructure (Kubernetes and VMs) with [Stack
Driver][stack-driver]

[grafana-cloud]: https://grafana.com/orgs/radicle/api-keys
[stack-driver]: https://console.cloud.google.com/monitoring?project=radicle-registry-dev
[radicle-grafana]: https://radicle.grafana.net

## Using Terraform

You need to install [sops][]. We use it as a data provider for secrets.

Terraform uses the [Google Application Default Credentials][google-adc] to
authenticate you. You can set the credentials by runnint [`gcloud auth
application-default login`][gcloud-login] or by setting the
`GOOGLE_APPLICATION_CREDENTIALS` environment variable.

Your Google Cloud account needs to have the appropriate permissions for the
`radicle-registry-dev` project.

[sops]: https://github.com/mozilla/sops
[gcloud-login]: https://cloud.google.com/sdk/gcloud/reference/auth/application-default/login
[google-adc]: https://cloud.google.com/docs/authentication/production#finding_credentials_automatically

## CI Build artifacts

### Binaries

Artifacts uploaded via the [buildkite agent](https://buildkite.com/docs/pipelines/artifacts)
are stored in a publicly readable Monadic-managed GCS bucket.

Artifacts uploaded by a `master` (that is, `$BUILDKITE_PIPELINE_DEFAULT_BRANCH`)
builds will have a predictable URL, e.g.:

```
https://builds.radicle.xyz/radicle-registry/master/$BUILDKITE_COMMIT/$ARTIFACT_PATH`
```

Artifacts uploaded by a git tag build will be uploaded to

```
https://builds.radicle.xyz/radicle-registry/$BUILDKITE_TAG/$ARTIFACT_PATH
```

All other artifacts are scoped by `$BUILDKITE_JOB_ID`, and best discovered
through the Buildkite UI or API. E.g.:

```
https://builds.radicle.xyz/radicle-registry/b2d9d6fd-cc6a-4c44-90e4-b07b5c50ee4c/$ARTIFACT_PATH`
```

### Runtimes and spec files

Runtimes and spec files built by the CI on the `master` branch are uploaded to a GCS bucket
[radicle-registry-runtime](https://console.cloud.google.com/storage/browser/radicle-registry-runtime).

The bucket contains specific files:
- WASM files of all the built runtimes.
  They are named `v<spec>_<impl>.wasm`, e.g. `v16_1.wasm`
- Spec files for the `dev` chain with the first impl of each spec.
  They are named `dev_v<spec>_0.json`, e.g. `dev_v16_0.json`
- Spec files for the `dev` chain with the latest impl of each spec.
  They are named `dev_v<spec>_latest.json`, e.g. `dev_v16_latest.json`
  These files are overwritten whenever a newer impl version is merged into `master`.
  When the newest impl version is 0, this file is the same as `dev_v<spec>_0.json`.

All the files are publicly available and anybody can download them.
The URL for each file is `https://storage.googleapis.com/radicle-registry-runtime/<file_name>`,
e.g. `https://storage.googleapis.com/radicle-registry-runtime/v16_1.wasm`.

To upload files to the bucket the CI uses a specially privileged service account
[radicle-registry-runtime-write](https://console.cloud.google.com/iam-admin/serviceaccounts/details/104261938534407474798?project=radicle-registry-dev).

## Runbook

### Resetting the Devnet chain

Resetting the devnet chain means that all blocks, transactions, and state
present in the devnet chain are discarded. The nodes running in the cluster will
start from a new genesis block.

To reset the devnet, follow these steps.

1. Remove the existing node deployments and associated data from the cluster
  ```bash
  kubectl delete deployments/miner statefulsets.apps/validator
  kubectl delete persistentvolumeclaims -l app=validator
  ```
  If you are resetting a different network, make sure that all other node
  deployments are deleted, too.
2. Update the node image in `./main.tf` to the latest version.
3. Run `terraform apply`
4. Verify that the devnet is connected and produces using our
   [dashboards][radicle-grafana].
