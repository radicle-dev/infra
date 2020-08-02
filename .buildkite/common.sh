function prepare_cache () {
    echo "--- Prepare cache"

    local -r target_cache="/cache/target"

    # Most of the caching is done through caching ./target
    export SCCACHE_CACHE_SIZE="1G"

    free_cache_space_kb=$(df --output=avail /cache | sed -n 2p)
    min_free_cache_kb=$(( 800 * 1024 ))
    echo "$(( free_cache_space_kb / 1024 )) MiB free space on /cache"
    if [[ $free_cache_space_kb -le $min_free_cache_kb ]]
    then
        echo "Reseting cache with rm -rf /cache/*"
        du -sh /cache/*
        rm -rf /cache/*
    fi
    mkdir -p "$target_cache"
    ln -s "$target_cache" ./target
}
