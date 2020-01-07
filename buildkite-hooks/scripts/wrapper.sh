#!/usr/bin/env bash
set -eou pipefail

declare script_name="${0##*/}"
script_name="${script_name%.*}" # strip extension

declare -rx RUST_BACKTRACE=1
exec "/usr/local/bin/buildkite-${script_name}-hook"
