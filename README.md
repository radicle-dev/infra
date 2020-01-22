# ci
CI infrastructure

## Linux build agents
### Caching

Each job container has a cache volume mounted at `/cache`. In general, the cache
volume is shared only between jobs for the same branch on the same runner. This
means that jobs on different runners or branches cannot share the cache. This
can be adjusted with the shared master cache (see below).

For branch builds the cache volume is created from a snapshot of the cache
volume of the master branch of the runner.

The cache volume has a quota of 8GiB. This value can be configured through
`CACHE_QUOTA_GiB` in `./linux/etc/buildkite-agent/hooks/command`.

#### Shared master cache

It is possible to configure a pipeline so that runners on the same machine share
the build cache of the builds of the default branch. This behavior is controlled
via the `SHARED_MASTER_CACHE` environment variable.

```yaml

.test: &test
  command: "tests.sh"
  env:
    SHARED_MASTER_CACHE: true
steps:
- branches: "!master"
  <<: *test
- branches: "master"
  concurrency: 1
  concurrency_group: 1
  <<: *test
```

To ensure that two runners don’t access the cache concurrently the concurrency
must be limited.

Note that `SHARED_MASTER_CACHE` cache must be enabled for both steps so that
branch builds also know to use the master cache.

### Building docker images

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
image, and dynamically assembles the `Dockerfile`. `img` uses the directory of
the `Dockerfile` as its context, i.e. you can only `ADD` files from there. It is
also possible to override the context by defining the `STEP_DOCKER_CONTEXT` env
variable.

The built image is tagged with the name given by `STEP_DOCKER_IMAGE` and the git
commit hash `BUILDKITE_COMMIT` as the tag. The agent pushes the image to a
registry deduced from `DOCKER_IMAGE`.

When building most of the [Buildkite environment variables][buildkite-env] are
available as [build arguments][docker-build-args].

The agent uses [`img`][img] to build the image.

[docker-build-args]: https://docs.docker.com/engine/reference/builder/#arg
[buildkite-env]: https://buildkite.com/docs/pipelines/environment-variables
[img]: https://github.com/genuinetools/img

### Secrets

The build agent probes for a file `.buildkite/secrets.yaml` in the source
checkout, and if it exists, attempts to decrypt it using [`sops`][sops] in
"dotenv" format into a file `.secrets` at the root of the source checkout.

Secrets are not available to pull requests builds.

Repositories making using of this feature must:

1. Create a new symmetric key in the GCP KMS.
2. Grant the `cloudkms.cryptoKeyEncrypterDecrypter` IAM role to all contributors
   who should be able to view / modify the secrets.
3. Grant the `cloudkms.cryptoKeyDecrypter` IAM role to the
   `buildkite-agent@opensourcecoin.iam.gserviceaccount.com` service account.
4. Create a `.sops.yaml` file at the root of the repository, which specifies the
   GCP KMS key to use for encrypting / decrypting the `.buildkite/secrets.yaml`
   file. See [sops documentation](https://github.com/mozilla/sops#using-sops-yaml-conf-to-select-kms-pgp-for-new-files)
   for details.

[sops]: https://github.com/mozilla/sops


## macOS build agents

For now we have one macOS host, a 2018 6-core i5 Mac mini (19C57) with
32Gb RAM and a 256GB SSD.

For security reasons it is configured to only build the `master` branch of the
official `radicle-upstream` repository at the moment.


### Agent setup

1. Buy or rent a Mac and set up latest macOS (Catalina 10.15.2)

2. Perform default user setup (part of macOS when you first turn it on),
   call the user: `buildkite`

3. Set up remote access via screen sharing
   `System Preferences` → `Sharing` → Check
   - [x] Screen Sharing

   The host will be reachable from any other Mac on the local network via the
   built-in Screen Sharing app.

4. Set up remote access via SSH via `System Preferences` → `Sharing` → Check
   - [x] Remote Login and add your SSH keys to `~/.ssh/authorized_keys`

5. Configure default account to automatically log in
   `System Preferences` → `Users & Groups` → `Login Options`
   → `Automatic login` → Choose `buildkite`

6. Prevent the Mac from going to sleep:
   `System Preferences` → `Energy Saver` → `Turn display off after`
   → Choose `never`
   - [x] "Prevent computer from sleeping automatically when the display is off"
   - [x] "Start up automatically after power failure"

7. Install [caffeine][caffeine] and configure it (right click on menu bar icon)
   to start on login:
   - [x] Automatically start Caffeine at login
   - [x] Activate Caffeine at launch
   - Default duration: Indefinitely

8. Install Xcode from App Store and configure it via the terminal:
   `xcode-select --install`

9. Install Homebrew
```
/usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"
```

10. Install some useful terminal utilities:
  `brew install htop neovim`

11. Set up Buildkite. The agent token can be retreived from the
    [buildkite website][buildkite] under `Agents` → `Agent Token`
    → `Reveal Agent Token`

```
brew tap buildkite/buildkite
brew install --token='!!!FILL_IN_AGENT_TOKEN!!!' buildkite-agent
```

12. Configure buildkite by copying config files from this repo `macos/` to the
    relevant paths:
    - `/usr/local/etc/buildkite-agent` (remember to fill in agent token!)
    - `/usr/local/etc/buildkite-agent/hooks/environment`

13. Create the build folder:
    `mkdir -p /Users/buildkite/buildkite-cache`

14. Set up radicle-upstream build dependencies
```
# rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
brew install openssl
brew install pkgconfig

# JavaScript toolchain
brew install yarn
```

15. Start Buildkite agent (this should also make sure it's started on reboot)
```
brew services start buildkite/buildkite/buildkite-agent
```


[caffeine]: http://lightheadsw.com/caffeine
[buildkite]: https://buildkite.com/organizations/monadic/agents
