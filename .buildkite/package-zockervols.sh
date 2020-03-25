#!/usr/bin/env bash
set -eou pipefail

pushd zockervols
trap popd EXIT

version="$(cargo read-manifest|jq -r .version)+${BUILDKITE_BUILD_NUMBER}"
deb="zockervols_${version}_amd64.deb"

cargo deb --deb-version="${version}"

if [[ "$BUILDKITE_BRANCH" == "master" ]]
then
    echo "Uploading ${deb} to bintray:"
    curl -sSf \
        -T "target/debian/${deb}"  \
        -u"${BINTRAY_API_KEY}" \
        -H"X-Bintray-Debian-Distribution: buster" \
        -H"X-Bintray-Debian-Component: main" \
        -H"X-Bintray-Debian-Architecture: amd64" \
        "https://api.bintray.com/content/oscoin/zockervols/zockervols/${version}/${deb}"
    echo

    echo "Publishing ${deb} on bintray:"
    curl -sSf -XPOST \
        -u"${BINTRAY_API_KEY}" \
       "https://api.bintray.com/content/oscoin/zockervols/zockervols/${version}/publish"
    echo
fi
