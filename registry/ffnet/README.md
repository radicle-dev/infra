FFnet infrastructure
--------------------

This module describes the infrastructure for the Radicle Registry FFnet.

* [Kubernetes cluster](./main.tf) on Google Cloud Platform
* Stateful [validator nodes](./validators.tf). The nodes have static P2P IDs and
  the P2P endpoints are publicly exposed through load balancers. See `terraform
  output` for the addresses
* [Miner deployment](./miners.tf) running on a dedicated node pool of
  preemptible [`c2` instances][c2-instances].
* A [Prometheus instance](./monitoring.tf) that collects metrics and forwards them
  to [Grafana Cloud](https://radicle.grafana.net).

[c2-instances]: https://cloud.google.com/compute/docs/machine-types#c2_machine_types
