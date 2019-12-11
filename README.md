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

## Building docker images

You can build and push docker images as part of a job by adding setting the
`BUILD_DOCKER_IMAGE` environment variable to `true`.

```yaml
steps:
- command: |-
    echo "hello world" > ./my_artifact
    mkdir image-build
    echo "FROM alpine" >> ./image-build/Dockerfile
    echo "ADD ./my_artifact ." >> ./image-build/Dockerfile
  env:
    BUILD_DOCKER_IMAGE: true
    BUILD_DOCKER_IMAGE_NAME: gcr.io/opensourcecoin/my-project
```

If building docker image is enabled the job command can create
`./image-build/Dockerfile` with the image build instructions. When the job
command has finished the agent will use this Docker file and the working
directory of the job as the context. This means that all artifacts created by
the job command can be added to the image.

The built image is tagged with the name given by `BUILD_DOCKER_IMAGE_NAME` and
the git commit hash `BUILDKITE_COMMIT` as the tag. The agent pushes the
image to the registry.

Building images is only available to the `oscoin` and `radicle-dev` Github
organizations. The image name is restricted to starting with
`gcr.io/opensourcecoin`.

When building most of the [Buildkite environment variables][buildkite-env] are
available as [build arguments][docker-build-args].

The agent uses [`img`][img] to build the image.

[docker-build-args]: https://docs.docker.com/engine/reference/builder/#arg
[buildkite-env]: https://buildkite.com/docs/pipelines/environment-variables
[img]: https://github.com/genuinetools/img
