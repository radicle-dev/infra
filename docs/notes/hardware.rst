Bare Metal
==========

This should be plenty for starters (EUR88):

+ Hetzner EX62-NVMe

  + 8-core Intel i9
  + 64GB DDR4
  + 2x1TB NVMe (*not* datacenter grade)
  + add. 1TB NVMe
  + add. 480GB SATA SSD (root)

+ Config:

  + Buster
  + SATA root
  + NVMes as striped zfs pool

Drawbacks:
----------

+ No private network


Cloud
=====

Run an instance group with preemptible instances.

Drawbacks:
----------

+ We'll often start with cold caches
