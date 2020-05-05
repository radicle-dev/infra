This is the documentation for Grafana alerts.

The Grafana alerts are defined as a part of a dashboard.
Its configuration is exported into a [JSON file](./grafana_alert_dashboard.json),
which can be used for manual recovery.
It's not applied automatically during creation of a Grafana instance.
**Remember to update the JSON file after any change in the alert dashboard!**

# Alerts

## Node is down
### Name
`Node is down alert`
### Goal
Check if the nodes in the cluster are running
### Trigger
All the nodes are checked every 15 seconds for their last `up` reading.
If the reading is 0 or the 15 seconds window contains no data, the alert starts pending.
If the alert is pending for 4 minutes, it's finally risen.
### Query
```promql
up { kubernetes_cluster="ffnet" }
```

## Blocks aren't imported
### Name
`Block import rate 7m low`
### Goal
Check if blocks are being mined and that the chain is growing
### Trigger
In the past 7 minutes the `substrate_block_height` metric has grown by less than 1.
This is triggered only if the node is running, i.e. the `up` metric has the value of `1`.
### Query
```promql
rate(substrate_block_height { kubernetes_cluster = "ffnet", status = "best" }[7m]) * 7 * 60
and on (instance) up{ kubernetes_cluster = "ffnet" } == 1
```

## Blocks are imported too slowly
### Name
`Block import rate 1h low`
### Goal
Check if blocks are being mined and that the chain is growing too slowly
### Trigger
In the past 1 hour the `substrate_block_height` metric has grown by less than 35.
This is triggered only if the node is running, i.e. the `up` metric has the value of `1`.
### Query
```promql
rate(substrate_block_height { kubernetes_cluster = "ffnet", status = "best" }[1h]) * 60 * 60
and on (instance) up{ kubernetes_cluster = "ffnet" } == 1
```
