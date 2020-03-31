Radicle Registry Infrastructure
===============================

This repository contains Terraform code that describes and manages the
infrastructure maintained by the Radicle Registry project.


Managed Infrastructure
----------------------

* Google Cloud Computing project `radicle-registry-dev`
* GKE cluster `radicle-registry-devnet`
  * StatefulSet of two validators that also serve as boot nodes. See `./devnet-validators.tf`
  * Deployment of mining nodes. See `./devnet-miners.tf`
  * Public telemetry server and dashboard for devnet. See `./devnet-telemetry.tf`
  * Prometheus to collect metrics from pods. See `./monitoring.tf`

Run `terraform output` for information about entry points.

Container Images
----------------

The `./images` folder holds buildscripts for container image used by the
infrastructure.

To build any of the images run `./images/<image-name>/build.sh`.

If you change an image make sure to bump the image version in the build script.


Using Terraform
---------------

To run `terraform` you need to set the environment variable.
```
export GOOGLE_APPLICATION_CREDENTIALS=./secrets/gcp-service-accounts/project-terraform.json
```

The credentials file belongs to the `project-terraform` service account and is
used to manage the infrastructure. The file is encrypted using
[`git-crypt`](https://github.com/AGWA/git-crypt). To be able to decrypt it you
GPG key must be added to the repo.
