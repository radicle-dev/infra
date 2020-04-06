Radicle Registry Infrastructure
===============================

This repository contains Terraform code that describes and manages the
infrastructure maintained by the Radicle Registry project.


Managed Infrastructure
----------------------

* Google Cloud Computing project `radicle-registry-dev`
* KMS key for managing
  * CI secrets in the [`radicle-registry`][radicle-registry] repository. See `./kms-ci.tf`.
  * Secrets in this repository. See `./kms-infra.tf`
* GKE cluster `radicle-registry-devnet`
  * StatefulSet of two validators that also serve as boot nodes. See `./devnet-validators.tf`
  * Deployment of mining nodes. See `./devnet-miners.tf`
  * Prometheus to collect metrics from pods. See `./monitoring.tf`

Run `terraform output` for information about entry points.

[radicle-registry]: https://github.com/radicle-dev/radicle-registry


Monitoring
----------

We use [Grafana Cloud][grafana-cloud] to monitor the Registry nodes. You can
find the Grafana instance at [`radicle.grafana.net`][radicle-grafaa]

We monitor the underlying infrastructure (Kubernetes and VMs) with [Stack
Driver][stack-driver]

[grafana-cloud]: https://grafana.com/orgs/radicle/api-keys
[stack-driver]: https://console.cloud.google.com/monitoring?project=radicle-registry-dev
[radicle-grafana]: https://radicle.grafana.net


Using Terraform
---------------

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
