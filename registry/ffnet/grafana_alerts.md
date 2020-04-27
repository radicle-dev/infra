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
Any node has metric `up` set to 0

## Blocks aren't imported
### Name
`Block import rate alert`
### Goal
Check if blocks are being mined and that the chain is growing
### Trigger
In the past 10 minutes the `substrate_block_height` metric has grown by less than 1
or in the past hour by less than 40.
