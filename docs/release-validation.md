# Version 1 release validation

Record every release candidate against the exact `.deb` checksum. A candidate
is not ready to publish until both machines pass their assigned rows and the
maintainer has verified the signature uploaded with the GitHub Release.

## Automated artifact gate

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
scripts/prepare-release-artifacts.sh target/release-artifacts
```

The artifact test checks the `amd64` package metadata and minimum dependencies,
the full installed payload, inert installation, and ownership-safe removal.
Archive the `.deb`, debug-symbol `.ddeb`, `.buildinfo`, `.changes`, and
`SHA256SUMS` from
`target/release-artifacts/`. Sign `SHA256SUMS` with the maintainer release key
and verify that signature before publishing.

## Clean lifecycle matrix

Use a disposable test user with representative manual forward and reverse
Window-switcher settings. Record the shortcut file before and after every row.

| Gate | Procedure | Required result |
| --- | --- | --- |
| Clean install | Install the signed `.deb`; do not run `enable` | Service is stopped and COSMIC's stock commands are unchanged |
| Explicit enable | Run `enable`, `status`, and `doctor` | User service runs; only the two semantic commands become app-owned |
| Upgrade | Install the next candidate over an enabled prior build | Integration remains enabled and user key bindings are unchanged |
| Manual edit | Change one semantic command after enablement, then disable | Manual value survives; the still-owned value is restored |
| Disable | Run `disable` twice | Service stops and stock switching is immediately usable |
| Remove | Re-enable, then remove without a prior disable | Removal restores owned values and leaves manual values intact |
| Purge | Purge after removal | No package-managed file remains; user preference data is not deleted |
| Unsupported session | Run lifecycle commands outside COSMIC Wayland | Commands reject the session without changing shortcuts or service state |
| Capability failure | Remove one required capability or stop recovery | No partial overlay appears; stock fallback preserves direction |

## Live behavior and performance matrix

Run the detailed contracts in [test-matrix.md](test-matrix.md), including Hold
Mode, Latch Mode, forward/reverse wrapping, quick release, Native Wayland and
XWayland Windows, DMA-BUF and forced SHM capture, minimization, fullscreen,
multiple workspaces/displays, fractional scaling, accessibility, and session
deactivation cleanup.

Measure and record:

- idle service capture count and sustained CPU usage;
- selection feedback latency against one display frame;
- overlay readiness latency against the 50 ms post-delay target;
- 10-Window 30 FPS capture behavior and input responsiveness under overload;
- selected-item freshness and fair round-robin progress for other visible rows.

## Candidate record

| Field | Development laptop | MSI Aegis ZS2 |
| --- | --- | --- |
| Release candidate / SHA-256 | Pending | Pending |
| Pop!_OS / COSMIC build | Pending | Pending |
| CPU / GPU / driver | Intel Core Ultra 9 288V / Lunar Lake / `xe` | Pending hardware capture |
| Display / scale / refresh | 2880×1800 / 175% / 120 Hz | Pending hardware capture |
| Clean lifecycle matrix | Pending | Pending |
| GPU, SHM, Native Wayland, XWayland | Pending | Pending |
| Performance targets | Pending | Pending |
| Validator and date | Pending | Pending |

Do not replace `Pending` with `Pass` without retaining the measurements and
manual observations for that exact checksum.

After both machines pass, update `release/v1-validation.json` with the exact
`.deb` SHA-256, `passed` statuses, evidence locations, validator, and ISO-8601
date. The tag workflow refuses to sign or publish while that machine-readable
attestation is pending or does not match the built package. Check it locally:

```sh
scripts/verify-release-attestation.sh target/release-artifacts/cosmic-window-switcher_0.2.0-1_amd64.deb
```

## GitHub Release gate

- The tag version matches the Debian and Cargo versions.
- The Release contains the `.deb`, debug-symbol `.ddeb`, `.buildinfo`,
  `.changes`, `SHA256SUMS`, and armored `SHA256SUMS.asc` maintainer signature.
- `gpg --verify` and `sha256sum --check` pass after downloading the Release.
- Release notes claim only Pop!_OS 24.04 COSMIC Wayland on `amd64`.
- No v2 control, apt repository, updater, telemetry, crash uploader, sandboxed
  package, or persisted thumbnail data is present.
