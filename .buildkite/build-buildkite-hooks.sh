#!/usr/bin/env bash
set -eoux pipefail

pushd buildkite-hooks

version="$(cargo read-manifest|jq -r .version)+${BUILDKITE_BUILD_NUMBER}"
deb="buildkite-hooks_${version}_amd64.deb"

echo "--- obey the stylez"
cargo fmt -- --check

echo "--- cargo test"
cargo test --all

echo "--- scripted tests"
test/test-cmd-signal
test/test-cmd-timeout

echo "--- cargo deb"
cargo deb --deb-version="${version}"

if [[ "$BUILDKITE_BRANCH" == "master" ]]
then
    set +x
    echo "--- Uploading ${deb} to bintray"
    curl -f \
        -T "target/debian/${deb}"  \
        -u"${BINTRAY_API_KEY}" \
        -H"X-Bintray-Debian-Distribution: buster" \
        -H"X-Bintray-Debian-Component: main" \
        -H"X-Bintray-Debian-Architecture: amd64" \
        "https://api.bintray.com/content/oscoin/buildkite-hooks/buildkite-hooks/${version}/${deb}"
    echo

    echo "--- Publishing ${deb} on bintray"
    curl -f -XPOST \
        -u"${BINTRAY_API_KEY}" \
       "https://api.bintray.com/content/oscoin/buildkite-hooks/buildkite-hooks/${version}/publish"
    echo
fi

popd
