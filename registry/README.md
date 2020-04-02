Radicle Registry Infrastructure
===============================

This repository contains Terraform code that describes and manages the
infrastructure maintained by the Radicle Registry project.


Managed Infrastructure
----------------------

* Google Cloud Computing project `radicle-registry-dev`
* KMS key for managing CI secrets in the [`radicle-registry`][radicle-registry]
  repository. See `./ci-secrets.tf`.
* GKE cluster `radicle-registry-devnet`
  * StatefulSet of two validators that also serve as boot nodes. See `./devnet-validators.tf`
  * Deployment of mining nodes. See `./devnet-miners.tf`
  * Public telemetry server and dashboard for devnet. See `./devnet-telemetry.tf`
  * Prometheus to collect metrics from pods. See `./monitoring.tf`

Run `terraform output` for information about entry points.

[radicle-registry]: https://github.com/radicle-dev/radicle-registry

Container Images
----------------

The `./images` folder holds build scripts for container image used by the
infrastructure.

To build any of the images run `./images/<image-name>/build.sh`.

If you change an image make sure to bump the image version in the build script.


Using Terraform
---------------

To run `terraform` you need a key file for the `project-terraform` service
account. Then set the `GOOGLE_APPLICATION_CREDENTIALS` environment variable to the
location of the file. Now you can use the `terraform` commands.
