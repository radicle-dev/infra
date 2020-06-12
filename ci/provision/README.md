# Provisioning a Buildkite agent

At the moment we only support provisioning a buildkite agent on GCP.

```
./create-gcp-instance
```

This creates a single GCP instance that runs the buildkite agent. It uses the
buildkite-hooks package build on a specific and configurable buildkite job.

To run this script your `gcloud` account needs to have permission to create
instances on the `opensourcecoin` GCP project and have access to the
`buildkite-agent` service account.

The provisioning can be customized via the environment variables defined in the
script.
