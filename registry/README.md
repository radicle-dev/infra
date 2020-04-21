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
* GKE cluster `radicle-registry-devnet` for running a devnet that we can play
  around with. See `./devnet`.
* GKE cluster `radicle-registry-ffnet` for running the FFnet information. See `./ffnet`.
* DNS zones in `./dns.tf`

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

Runbook
-------

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
