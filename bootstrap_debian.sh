#!/usr/bin/env bash
set -eou pipefail

declare -r VERSION="${__VERSION}"
declare -ra STORAGE_DEVICES=("${__STORAGE_DEVICES}")
declare -r CONFIG_BUCKET="${__CONFIG_BUCKET}"

function apt_keys {
    # FIXME:
    #   should use this b/c signature attacks. doesn't seem to replicate, tho
    #local keysrv=hkps://keys.openpgp.org
    local keysrv=hkps://keyserver.ubuntu.com
    local keys=(
        9DC858229FC7DD38854AE2D88D81803C0EBFCD88 # Docker
        8756C4F765C9AC3CB6B85D62379CE192D401AB61 # Bintray (zockervols)
        32A37959C2FA5C3C99EFBC32A79206696452D198 # Buildkite
        9FDC0CB63708CF803696E2DCD0B37B826063F3ED # SuSE (kata containers)
        54A647F9048D5688D7DA2ABE6A030B21BA07F4FB # Google (gce sdk)
    )

    apt-key adv --keyserver="$keysrv" --recv-keys "${keys[@]}"
}

function apt_install {
    set -x
    DEBIAN_FRONTEND=noninteractive apt-get install -y "$@"
    set +x
}

function apt_packages {
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
        zockervols
}

function zfs_exists {
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

function storage {
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
            "atime,clone,create,compression,destroy,exec,mount,mountpoint,promote,quota,rename,setuid,snapshot" \
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

function config {
    local config_tarball="buildkite-agent-${VERSION}.tar.gz"

    gsutil cp "gs://${CONFIG_BUCKET}/${config_tarball}" /root

    pushd /

    tar xvfz "/root/${config_tarball}"

    while IFS= read -r -d '' ciph
    do
        base64 --decode -i "$ciph" | \
            gcloud kms decrypt \
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
    chgrp buildkite-builder /etc/gce/*

    chown -R root:root /etc/docker
    chmod 600 /etc/docker/daemon.json

    chown -R root:root /etc/systemd/system
    find /etc/systemd/system -type f -exec chmod 644 {} \;

    chown root:root /etc/sudoers.d/*
    chmod 440 /etc/sudoers.d/*

    popd
}

function users_groups {
    if ! getent passwd buildkite-agent > /dev/null
    then
        useradd --user-group --system buildkite-builder
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
}

function services {
    systemctl daemon-reload

    systemctl enable docker
    systemctl enable zockervols.socket
    systemctl enable docker-volume-prune.timer

    systemctl restart docker
    systemctl start zockervols.socket
    systemctl start docker-volume-prune.timer

    local -i cpus
    local -i agents

    cpus=$(nproc)
    cpus=$((cpus < 2 ? 2 : cpus))

    agents=$((cpus / 2))

    for i in $(seq 0 $((agents - 1)))
    do
        for cmd in enable start
        do
            systemctl $cmd "buildkite-agent@${i}"
        done
    done
}

function metadata_concealment {
    local rule=(
        "--in-interface=docker0"
        "--destination=169.254.169.254"
        "--protocol=tcp"
        "--jump=REJECT"
    )
    iptables -D DOCKER-USER "${rule[@]}" || true
    iptables -I DOCKER-USER 1 "${rule[@]}"
    netfilter-persistent save
}

function main {
    users_groups
    config

    apt_keys
    apt-get update

    storage
    apt_packages
    services

    metadata_concealment
}

main
