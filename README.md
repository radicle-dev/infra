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

Linux builds run inside docker containers. The image to use for the build step
is specified via the `DOCKER_IMAGE` environment variable of the step. The image
may also be built on the build agent itself, before executing the build step. To
do this, specify an environment variable `DOCKER_FILE` which points to a
`Dockerfile` relative to the repository root.

Note that `DOCKER_IMAGE` takes precedence over `DOCKER_FILE` -- if `docker pull
$DOCKER_IMAGE` succeeds, no new image is built.

Only `DOCKER_IMAGE`s from the `gcr.io/opensourcecoin` repository are permitted.
Images built by the agent are pushed to `gcr.io/opensourcecoin/${BUILDKITE_PIPELINE_SLUG}-build:${BUILDKITE_COMMIT}`
if no `DOCKER_IMAGE` is given, and to `${DOCKER_IMAGE}:${BUILDKITE_COMMIT}`
otherwise.

```yaml
steps:
- command: cargo test
  env:
    DOCKER_FILE: docker/build-image/Dockerfile
    # After the image was built successfully, save build minutes by pinning it
    # to its SHA256 hash:
    # DOCKER_IMAGE: gcr.io/opensourcecoin/my-project-build@sha256:51ec4db1da1870e753610209880f3ff1759ba54149493cf3118b47a84edbc75b
```

It is also possible to define build steps which build and push docker images. To
do so, define `STEP_DOCKER_FILE` and `STEP_DOCKER_IMAGE`:

```yaml
steps:
- command: |-
    echo "hello world" > ./my_artifact
    mkdir image-build
    mv my_artifact image-build
    echo "FROM alpine" >> ./image-build/Dockerfile
    echo "ADD ./my_artifact ." >> ./image-build/Dockerfile
  env:
    STEP_DOCKER_FILE: image-build/Dockerfile
    STEP_DOCKER_IMAGE: gcr.io/opensourcecoin/my-project
```

The step in this example creates a build artifact to be packaged in the docker
image, and dynamically assembles the `Dockerfile`. `img` sees only the directory
where the `Dockerfile` lives, so make sure all the artifacts are copied there.

The built image is tagged with the name given by `STEP_DOCKER_IMAGE` and the git
commit hash `BUILDKITE_COMMIT` as the tag. The agent pushes the image to a
registry deduced from `DOCKER_IMAGE`.

When building most of the [Buildkite environment variables][buildkite-env] are
available as [build arguments][docker-build-args].

The agent uses [`img`][img] to build the image.

[docker-build-args]: https://docs.docker.com/engine/reference/builder/#arg
[buildkite-env]: https://buildkite.com/docs/pipelines/environment-variables
[img]: https://github.com/genuinetools/img
