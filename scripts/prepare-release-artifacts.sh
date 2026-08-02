#!/bin/bash

set -euo pipefail

repository=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
destination=${1:-"$repository/target/release-artifacts"}
package=$("$repository/scripts/build-deb.sh" "$destination")

"$repository/tests/package-artifact.sh" "$package"

cd "$destination"
mapfile -t artifacts < <(find . -maxdepth 1 -type f \
    \( -name '*.deb' -o -name '*.ddeb' -o -name '*.buildinfo' -o -name '*.changes' \) \
    -printf '%f\n' | sort)
[[ ${#artifacts[@]} -gt 0 ]] || {
    echo "no release artifacts were produced" >&2
    exit 1
}
sha256sum "${artifacts[@]}" >SHA256SUMS
sha256sum --check SHA256SUMS
