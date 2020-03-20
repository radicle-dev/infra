#!/usr/bin/env bash
set -eou pipefail

declare -r VERSION="${__VERSION}"
declare -ra STORAGE_DEVICES=("${__STORAGE_DEVICES}")
declare -r CONFIG_BUCKET="${__CONFIG_BUCKET}"

apt_keys() {
    # FIXME:
    #   should use this b/c signature attacks. doesn't seem to replicate, tho
    #local keysrv=hkps://keys.openpgp.org
    local keysrv=hkps://keyserver.ubuntu.com
    local keys=(
        9DC858229FC7DD38854AE2D88D81803C0EBFCD88 # Docker
        8756C4F765C9AC3CB6B85D62379CE192D401AB61 # Bintray (zockervols, sops)
        32A37959C2FA5C3C99EFBC32A79206696452D198 # Buildkite
        9FDC0CB63708CF803696E2DCD0B37B826063F3ED # SuSE (kata containers)
        54A647F9048D5688D7DA2ABE6A030B21BA07F4FB # Google (gce sdk)
    )

    set -x
    apt-key adv --keyserver="$keysrv" --recv-keys "${keys[@]}"
    set +x
}

apt_install() {
    set -x
    DEBIAN_FRONTEND=noninteractive apt-get install -y "$@"
    set +x
}

apt_packages() {
    apt_install \
        buildkite-agent \
        ca-certificates \
        containerd.io \
        curl \
        docker-ce \
        docker-ce-cli \
        gnupg2 \
        google-cloud-sdk \
        iptables-persistent \
        kata-proxy \
        kata-runtime \
        kata-shim \
        jq \
        sops \
        zockervols
}

zfs_exists() {
    local cmd="$1"
    local name="$2"

    case $cmd in
        zpool|zfs)
            $cmd list -H "$name" | wc -l || true
            ;;
        *)
            echo "Bad cmd: $cmd"
            exit 1
            ;;
    esac
}

storage() {
    apt_install --no-install-recommends zfs-dkms
    modprobe zfs
    apt_install zfsutils-linux

    # udev rules seem to not apply .. sometimes
    # we need this to be 666 for zfs delegations to work (it should be safe as
    # per https://github.com/zfsonlinux/zfs/pull/4487, as zfs performs all
    # access checks itself)
    chmod 666 /dev/zfs

    [[ "$(zfs_exists zpool tank)" == 1 ]] || {
        set -x
        # shellcheck disable=SC2068
        zpool create tank ${STORAGE_DEVICES[@]}
        set +x
    }

    [[ "$(zfs_exists zfs tank/docker)" == 1 ]] || {
        set -x
        zfs create \
            -o atime=off \
            -o compression=on \
            -o mountpoint=/mnt/docker \
            tank/docker
        set +x
    }

    [[ "$(zfs_exists zfs tank/zocker)" == 1 ]] || {
        set -x
        zfs create \
            -o atime=off \
            -o compression=on \
            -o exec=on \
            -o setuid=off \
            -o mountpoint=/mnt/zocker \
            tank/zocker
        zfs allow -g buildkite-builder \
            "atime,clone,create,compression,destroy,exec,mount,mountpoint,promote,quota,refquota,rename,setuid,snapshot" \
            tank/zocker
        set +x
    }

    [[ "$(zfs_exists zfs tank/builds)" == 1 ]] || {
        set -x
        zfs create \
            -o atime=off \
            -o compression=on \
            -o exec=on \
            -o setuid=off \
            -o mountpoint=/mnt/builds \
            tank/builds
        set +x
    }

    chown buildkite-builder:buildkite-builder /mnt/zocker
    chown buildkite-builder:buildkite-agent   /mnt/builds
    chmod 775 /mnt/builds
}

config() {
    local config_tarball="buildkite-agent-${VERSION}.tar.gz"

    set -x
    gsutil cp "gs://${CONFIG_BUCKET}/${config_tarball}" /root

    tar -C / -xvf "/root/${config_tarball}"

    while IFS= read -r -d '' ciph
    do
        base64 --decode -i "$ciph" \
        | gcloud kms decrypt \
            --keyring=buildkite \
            --key=bootstrap \
            --location=global \
            --ciphertext-file=- \
            --plaintext-file="${ciph%.asc}"
    done < <(find /etc -type f -name "*.asc" -print0)

    chown -R buildkite-agent:buildkite-agent /etc/buildkite-agent
    find /etc/buildkite-agent -maxdepth 1 -type f -exec chmod 600 {} \;
    chmod 755 /etc/buildkite-agent/hooks/*

    chmod 440 /etc/gce/*
    chgrp buildkite-agent /etc/gce/*

    chown -R root:root /etc/docker
    chmod 600 /etc/docker/daemon.json

    chown -R root:root /etc/systemd/system
    find /etc/systemd/system -type f -exec chmod 644 {} \;
    chgrp buildkite-agent /etc/systemd/system/docker-volume-prune.sh
    chmod 754 /etc/systemd/system/docker-volume-prune.sh

    chown -R root:root /etc/sudoers.d
    chmod 440 /etc/sudoers.d/*

    set +x
}

docker_gcr_auth() {
    set -x
    sudo -u buildkite-agent gcloud auth configure-docker --quiet
    sudo -u buildkite-agent gcloud auth activate-service-account \
        buildkite-agent@opensourcecoin.iam.gserviceaccount.com \
        --key-file=/etc/gce/cred.json \
        --quiet
    set +x
}

users_groups() {
    set -x
    if ! getent passwd buildkite-builder > /dev/null
    then
        useradd --user-group --system --uid 998 buildkite-builder
    fi

    if ! getent group docker > /dev/null
    then
        groupadd --system docker
    fi

    if ! getent passwd buildkite-agent > /dev/null
    then
        useradd \
            --user-group \
            --home-dir /var/lib/buildkite-agent \
            --groups docker,buildkite-builder \
            --system \
            buildkite-agent
        mkdir -p /var/lib/buildkite-agent
        chown -R buildkite-agent:buildkite-agent /var/lib/buildkite-agent
    else
        usermod -aG docker,buildkite-builder buildkite-agent
    fi
    set +x
}

services() {
    local units=(
        docker
        zockervols.socket
        docker-volume-prune.timer
        docker-system-prune.timer
    )

    local -i cpus agents

    cpus=$(nproc)
    cpus=$((cpus < 2 ? 2 : cpus))

    agents=$((cpus / 2))
    for i in $(seq 0 $((agents - 1)))
    do
        units+=("buildkite-agent@${i}")
    done

    set -x
    systemctl daemon-reload

    for unit in "${units[@]}"
    do
        for cmd in enable restart
        do
            systemctl $cmd "$unit"
        done
    done
    set +x
}

metadata_concealment() {
    local rule=(
        "--in-interface=docker0"
        "--destination=169.254.169.254"
        "--protocol=tcp"
        "--jump=REJECT"
    )

    set -x
    iptables -D DOCKER-USER "${rule[@]}" || true
    iptables -I DOCKER-USER 1 "${rule[@]}"
    netfilter-persistent save
    set +x
}

main() {
    echo
    echo 'Users & Groups'
    echo
    users_groups

    echo
    echo 'Static Configuration'
    echo
    config

    echo
    echo 'apt Setup'
    echo
    apt_keys
    apt-get update

    echo
    echo 'Storage'
    echo
    storage

    echo
    echo 'Extra apt Packages'
    echo
    apt_packages

    echo
    echo 'Docker GCR Auth'
    echo
    docker_gcr_auth

    echo
    echo 'systemd Services'
    echo
    services

    echo
    echo 'Metadata Concealment'
    echo
    metadata_concealment
}

main "$@"
