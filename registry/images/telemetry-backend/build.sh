#!/usr/bin/env bash

set -euo pipefail

image_name="gcr.io/opensourcecoin/radicle-registry/telemetry-backend"
image_version=v1

dir=$(dirname "${BASH_SOURCE[0]}")
docker build "$dir" --tag "$image_name:$image_version"
