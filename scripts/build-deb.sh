#!/bin/bash

set -euo pipefail

repository=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
destination=${1:-"$repository/target/debian"}
version=$(dpkg-parsechangelog -l"$repository/debian/changelog" -SVersion)
architecture=$(dpkg-architecture -qDEB_HOST_ARCH)
stem="cosmic-window-switcher_${version}_${architecture}"
debug_stem="cosmic-window-switcher-dbgsym_${version}_${architecture}"

mkdir -p "$destination"
cd "$repository"
dpkg-buildpackage -b -us -uc -d >&2

for extension in deb buildinfo changes; do
    source_file="$repository/../${stem}.${extension}"
    if [[ -f $source_file ]]; then
        install -m 0644 "$source_file" "$destination/${stem}.${extension}"
    fi
done

if [[ -f $repository/../${debug_stem}.ddeb ]]; then
    install -m 0644 \
        "$repository/../${debug_stem}.ddeb" \
        "$destination/${debug_stem}.ddeb"
fi

[[ -f $destination/${stem}.deb ]] || {
    echo "package build did not produce ${stem}.deb" >&2
    exit 1
}

printf '%s\n' "$destination/${stem}.deb"
