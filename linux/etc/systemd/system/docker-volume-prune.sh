#!/usr/bin/env bash
set -eou pipefail

expiration_date="$(date --utc --date='1 week ago' +'%Y-%m-%dT00:00:00Z')"

mapfile -t prune < <(
    docker volume ls --filter=label=build_cache --format='{{ .Name }}' \
    | xargs --no-run-if-empty docker volume inspect \
    | jq -rM --arg date "$expiration_date" \
        '. | map(select(.Mountpoint == "none")) | map(select(.CreatedAt < $date)) | map(.Name) | .[]'
)

if [ ${#prune[@]} -gt 0 ]
then
    set +e
    for vol in "${prune[@]}"
    do
        set -x
        docker volume rm "$vol"
        set +x
    done
else
    echo "No prunable docker volumes found."
fi
