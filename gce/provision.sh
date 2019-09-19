#!/usr/bin/env bash
set -eou pipefail

: "${GCE_REGION:=europe-west3}"
: "${GCE_ZONE:=$GCE_REGION-a}"
: "${GCE_NUM_SSDS:=3}"
: "${GCE_MACHINE_TYPE:=n1-standard-4}"

function realrealpath {
    realpath "$@" 2>/dev/null || grealpath "$@"
}

VERSION="$(git rev-parse --short HEAD)"
CONFIG_BUCKET="eu.artifacts.opensourcecoin.appspot.com/configs"
declare -i NUM_SSDS
NUM_SSDS=$((GCE_NUM_SSDS > 8 ? 8 : (GCE_NUM_SSDS < 1 ? 1 : GCE_NUM_SSDS)))
BOOTSTRAP="$(realrealpath "$(dirname "${BASH_SOURCE[0]}")/bootstrap_debian.sh")"

function __wait_boot {
    # TODO: try connecting a few times
    sleep 15
}

function prepare_static_config {
    echo "Preparing static configuration..."

    local config_archive="buildkite-agent-${VERSION}.tar.gz"

    set -x
    git archive --format=tar.gz --output="${config_archive}" HEAD etc
    gsutil cp "${config_archive}" "gs://${CONFIG_BUCKET}/${config_archive}"
    set +x
}


# shellcheck disable=SC2034
# shellcheck disable=SC2016
function prepare_boostrap {
    echo "Preparing startup-script..."

    local -a storage_devices
    for i in $(seq 1 $NUM_SSDS)
    do
        storage_devices+=("/dev/nvme0n${i}")
    done

    local bootstrap_base
    bootstrap_base="$(realrealpath "$(dirname "${BASH_SOURCE[0]}")/../bootstrap_debian.sh")"

    local -x __VERSION="$VERSION"
    local -x __STORAGE_DEVICES="${storage_devices[*]}"
    local -x __CONFIG_BUCKET="${CONFIG_BUCKET}"

    set -x
    envsubst '$__VERSION:$__STORAGE_DEVICES:$__CONFIG_BUCKET' \
        < "$bootstrap_base" > "$BOOTSTRAP"
    set +x
}

function create_base_image {
    echo "Creating base image..."

    local -i vmx base
    vmx="$(gcloud compute images list --filter='name=debian-buster-vmx' --format='value(name)' | wc -l)"
    base="$(gcloud compute images list --filter="name=buildkite-base-${VERSION}" --format='value(name)' | wc -l)"

    if [[ $vmx -lt 1 ]]
    then
        set -x
        gcloud beta compute images create \
            "debian-buster-vmx" \
            --source-image-project=debian-cloud \
            --source-image-family=debian-10 \
            --licenses=https://www.googleapis.com/compute/v1/projects/vm-options/global/licenses/enable-vmx \
            --storage-location="${GCE_REGION}"
        set +x
    fi

    if [[ $base -lt 1 ]]
    then
        set -x

        gcloud compute instances create \
            "buildkite-base-builder-${VERSION}" \
            --image="debian-buster-vmx" \
            --machine-type="n1-standard-1" \
            --zone="${GCE_ZONE}"

        __wait_boot "buildkite-base-builder-${VERSION}"

        gcloud compute ssh \
            "buildkite-base-builder-${VERSION}" \
            --zone="${GCE_ZONE}" \
            --command='
set -e;
declare -rx DEBIAN_FRONTEND=noninteractive;
sudo apt-get update;
sudo apt-get upgrade -y;
sudo apt-get install -y linux-headers-cloud-amd64 linux-image-cloud-amd64;
sudo apt-get install -y dkms spl-dkms;
'
        set +e
        gcloud compute ssh \
            "buildkite-base-builder-${VERSION}" \
            --zone="${GCE_ZONE}" \
            --command="sudo reboot"
        set -e

        __wait_boot "buildkite-base-builder-${VERSION}"

        gcloud compute ssh \
            "buildkite-base-builder-${VERSION}" \
            --zone="${GCE_ZONE}" \
            --command="sudo apt-get autoremove -y"
        gcloud beta compute images create \
            "buildkite-base-${VERSION}" \
            --source-disk="buildkite-base-builder-${VERSION}" \
            --source-disk-zone="${GCE_ZONE}" \
            --family="buildkite-base" \
            --storage-location="${GCE_REGION}" \
            --force
        gcloud compute instances delete \
            "buildkite-base-builder-${VERSION}" \
            --zone="${GCE_ZONE}" \
            --quiet

        set +x
    fi
}

# TODO: VPC
function create_instances {
    local -i want have need

    want=${1:-1}
    have="$(gcloud compute instances list --filter="name=buildkite-agent-${VERSION}" --format='value(name)' | wc -l)"

    if [[ $have -lt $want ]]
    then
        need=$((want - have))
        echo "Creating $need instances..."

        local -a ssds
        for _ in $(seq 1 $NUM_SSDS)
        do
            ssds+=("--local-ssd=interface=nvme")
        done

        for i in $(seq "$have" $((have + need - 1)))
        do
            set -x
            gcloud compute instances create \
                "buildkite-agent-${VERSION}-${i}" \
                --image="buildkite-base-${VERSION}" \
                --machine-type="${GCE_MACHINE_TYPE}" \
                --min-cpu-platform="Intel Broadwell" \
                --zone="${GCE_ZONE}" \
                --service-account="buildkite-agent-bootstrap@opensourcecoin.iam.gserviceaccount.com" \
                --scopes="default,https://www.googleapis.com/auth/cloudkms" \
                "${ssds[@]}"

            __wait_boot "buildkite-agent-${VERSION}-${i}"

            gcloud compute ssh \
                "buildkite-agent-${VERSION}-${i}" \
                --zone="${GCE_ZONE}" \
                -- 'sudo bash -s' < "${BOOTSTRAP}"
            set +x
        done
    fi

    echo
    echo "Buildkite agents are started with the production=false tag."
    echo "You may wish to edit /etc/buildkite-agent/buildkite-agent.cfg"
    echo "manually to schedule actual builds on the newly provisioned agents."
    echo
}

function main {
    prepare_static_config
    prepare_boostrap
    create_base_image
    create_instances "$@"
}
main "$@"
