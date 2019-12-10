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
