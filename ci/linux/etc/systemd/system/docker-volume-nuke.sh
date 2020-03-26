#!/usr/bin/env bash
set -eou pipefail

# Get all docker image IDs and remove them.
define -r images = $(docker images --format='{{.ID}}')
echo "Nuking images: ${images}"
docker rmi images
