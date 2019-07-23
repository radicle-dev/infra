Linux
=====

Generally: use buildkite "command" hook to spawn build steps in docker
container.

+ **"Trusted"** builds (i.e. from whitelisted repo) use plain docker.
+ **"Untrusted"** builds (i.e. outside PRs) use `kata-container`_ runtime (KVM).

   *This is only really relevant for bare metal agents.*

Docker security considerations:
-------------------------------

+ Drop ALL capabilities (``--cap-drop=ALL``)
+ Run as ``buildkite-agent`` user:group (``--user=$(id -u):$(id -g)``).

   *Note: perhaps create another user and add to agent group for rw access to
   bind mounts?*

+ Mount root ro (``--read-only``).

   *Note: cargo package index then needs to reside on the host (cf.
   oscoin/ci#1).*

+ Limit kernel memory (``--kernel-memory``)?
+ Put filesystem quota in place for bind-mounted directories (implies ZFS)
+ Only accept images from ``gcr.io/opensourcecoin``
+ ``--security-opt no-new-privileges``

.. _kata-container: https://katacontainers.io
