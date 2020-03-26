#!/usr/bin/env bash
set -eou pipefail

declare -ri MIN_FREE_PERCENT=5

for fs in $(zfs list -Ho name)
do
    mapfile -t usage < <(zfs get refquota,available -Hpo value "$fs")
    declare -i quota=${usage[0]}
    declare -i avail=${usage[1]}

    declare -i quota_percentage=$(((quota / 100) * MIN_FREE_PERCENT))
    if [[ $avail < $quota_percentage ]]
    then
        echo "${fs} is approaching quota limit: ${avail} of ${quota}"
        zfs destroy -Rrv "${fs}" || true
	echo
    fi
done

