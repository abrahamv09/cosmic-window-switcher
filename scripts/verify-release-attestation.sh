#!/bin/bash

set -euo pipefail

repository=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
package=${1:?usage: scripts/verify-release-attestation.sh PACKAGE.deb}
attestation=${2:-"$repository/release/v1-validation.json"}

python3 - "$package" "$attestation" <<'PY'
import datetime
import hashlib
import json
import pathlib
import subprocess
import sys

package = pathlib.Path(sys.argv[1])
attestation_path = pathlib.Path(sys.argv[2])
attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
package_version = subprocess.run(
    ["dpkg-deb", "--field", package, "Version"],
    check=True,
    capture_output=True,
    text=True,
).stdout.strip().split("-", 1)[0]
digest = hashlib.sha256(package.read_bytes()).hexdigest()

errors = []
if attestation.get("version") != package_version:
    errors.append("attested version does not match the package")
if attestation.get("package_sha256") != digest:
    errors.append("attested SHA-256 does not match the package")
for machine in ("development_laptop", "msi_aegis_zs2"):
    result = attestation.get(machine, {})
    if result.get("status") != "passed":
        errors.append(f"{machine} validation is not passed")
    if not result.get("evidence") or result.get("evidence") == "PENDING":
        errors.append(f"{machine} validation evidence is missing")
if not attestation.get("attested_by") or attestation.get("attested_by") == "PENDING":
    errors.append("attester identity is missing")
try:
    datetime.date.fromisoformat(attestation.get("attested_on", ""))
except ValueError:
    errors.append("attestation date is not ISO-8601")

if errors:
    for error in errors:
        print(f"release attestation failed: {error}", file=sys.stderr)
    raise SystemExit(1)

print(f"release attestation passed for {package.name}")
PY
