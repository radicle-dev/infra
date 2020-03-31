#!/usr/bin/env bash

set -euo pipefail

image_name="gcr.io/opensourcecoin/radicle-registry/telemetry-frontend"
image_version=v6

dir=$(dirname "${BASH_SOURCE[0]}")
docker build "$dir" --tag "$image_name:$image_version"
