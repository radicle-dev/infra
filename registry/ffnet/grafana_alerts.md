This is the documentation for Grafana alerts.

The Grafana alerts are defined as a part of a dashboard.
Its configuration is exported into a [JSON file](./grafana_alert_dashboard.json),
which can be used for manual recovery.
It's not applied automatically during creation of a Grafana instance.
**Remember to update the JSON file after any change in the alert dashboard!**

# Alerts

## Node is down
### Name
`Node is down`
### Goal
Check if the nodes in the cluster are running
### Trigger
All the nodes are checked every 10 seconds for their `up` reading.
If any reading in a 10 second window is 0 or it contains no data, the alert starts pending.
If the alert is pending for 4 minutes, it's finally risen.
### Query
```promql
up { kubernetes_cluster="ffnet" }
```

## Node is not connected to its peers
### Name
`Peer connections low`
### Goal
Check if the nodes in connected to at least validator nodes and the RPC server.
### Trigger
All the nodes are checked every 10 seconds for their `substrate_sub_libp2p_peers_count` reading.
If this reading is below threshold for 1 minute, the alert is risen.
Miner nodes aren't guaranteed to be up, so it's fine when a node is connected only with validators
and an RPC server.
The thresholds depend on a node's `kubernetes_pod_label_app` label:
- `miner` - 3 (sum of validators and RPC servers)
- `validator` or `rpc-server` - 2 (sum of validators and RPC servers minus self)
### Query
```promql
substrate_sub_libp2p_peers_count{kubernetes_cluster="ffnet", kubernetes_pod_label_app="miner"}
```
```promql
substrate_sub_libp2p_peers_count{kubernetes_cluster="ffnet", kubernetes_pod_label_app=~"validator|rpc-server"}
```

## Blocks are imported in invalid rate in 11 minute window
### Name
`Block import rate 11m invalid`
### Goal
Check if blocks are being mined correctly and that the chain is growing at a sensible rate
### Trigger
In the past 11 minutes the `substrate_block_height` metric of a validator node has grown
by less than 1 or more than 25. This is triggered only if the node is running, i.e. the `up` metric
has the value of `1` and in the past 12 minutes the node wasn't major syncing.
### Query
```promql
rate(substrate_block_height { kubernetes_cluster = "ffnet", status = "best", kubernetes_pod_label_app="validator" }[11m]) * 11 * 60 and on (instance) up{ kubernetes_cluster = "ffnet" } == 1 and on (instance) (max_over_time(substrate_sub_libp2p_is_major_syncing{ kubernetes_cluster = "ffnet" }[12m]) == 0)
```

## Blocks are imported in invalid rate in 60 minute window
### Name
`Block import rate 1h invalid`
### Goal
Check if blocks are being mined correctly and that the chain is growing at a sensible rate
### Trigger
In the past 1 hour the `substrate_block_height` metric of a validator node has grown
by less than 35 or more than 80. This is triggered only if the node is running, i.e. the `up` metric
has the value of `1` and in the past 1 hour and 1 minute the node wasn't major syncing.
### Query
```promql
rate(substrate_block_height { kubernetes_cluster = "ffnet", status = "best", kubernetes_pod_label_app="validator" }[1h]) * 60 * 60 and on (instance) up{ kubernetes_cluster = "ffnet" } == 1 and on (instance) (max_over_time(substrate_sub_libp2p_is_major_syncing{ kubernetes_cluster = "ffnet" }[61m]) == 0)
```
