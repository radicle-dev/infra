#!/bin/bash

set -exuo pipefail

pod_index=$(echo "$KUBERNETES_POD_NAME" | sed -E 's/^.*([0-9]+)$/\1/')

declare -a extra_args
if [[ "$pod_index" = "0" ]]; then
  extra_args=(
    # Boot node id: QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR
    --node-key 0000000000000000000000000000000000000000000000000000000000000001
  )
fi

exec radicle-registry-node \
  --data-path /data \
  --chain devnet \
  --unsafe-rpc-external \
  --prometheus-external \
  --bootnodes /dns4/validator-0.validator.devnet.svc.cluster.local/tcp/30333/p2p/QmRpheLN4JWdAnY7HGJfWFNbfkQCb6tFf4vvA6hgjMZKrR \
  "${extra_args[@]}"
