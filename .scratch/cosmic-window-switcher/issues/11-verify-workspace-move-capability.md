# 11 — Move one Window through a verified workspace capability

**What to build:** A narrow end-to-end command that moves one chosen Window to another workspace only through a capability the running COSMIC compositor both advertises and honors. This resolves the known protocol mismatch before any drag UI depends on it.

**Blocked by:** 01 — Prove a two-Window COSMIC switch.

**Status:** ready-for-agent

- [x] Runtime probing records the advertised management protocol version and workspace-move capabilities on the packaged compositor.
- [x] The client never issues a request whose matching capability is not advertised.
- [ ] A supported request moves one test Window exactly once and reports the resulting workspace membership.
- [ ] An ignored or rejected request leaves the Window in its original workspace and reports a clear failed capability.
- [x] If the advertisement/handler mismatch reproduces, a minimal first-party-source-backed upstream report or patch is prepared and tracked.
- [ ] This ticket remains incomplete until the installed compositor exposes a verified advertised-and-honored move path.
- [x] The probe covers both spanning and separate-display workspace configurations where the machine supports them.

## Comments

- Implemented a capability-gated `probe-workspace-move` command. Inventory mode
  reports protocol versions, raw management capabilities, workspace
  group/output topology, workspace selectors, Window ids, and observed
  membership without issuing a move.
- Live validation on 2026-07-29 reproduced the blocker on packaged
  `cosmic-comp 1.0.0` (`ffeda3375a7e60ace6ae64b19432f1f0c1fc1034`):
  manager v4 advertised capabilities `[1, 2, 3, 4, 6]`, so the probe rejected
  the operation before sending a request. Capability 8 was absent.
- The live machine exposed one output, so the probe reported its
  single-output topology. The same inventory and output-selection path handles
  one multi-output spanning group and multiple separate-display groups when
  those configurations are available.
- Prepared
  `.scratch/cosmic-window-switcher/upstream/cosmic-comp-workspace-move-capability-mismatch.md`
  with the reproduction, first-party source evidence, and minimal correction.
- Ticket remains incomplete: no installed compositor capability can yet be both
  advertised and honored, and the package also emitted no initial ext-workspace
  membership for the discovered Windows.
