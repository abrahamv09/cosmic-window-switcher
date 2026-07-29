# 11 — Move one Window through a verified workspace capability

**What to build:** A narrow end-to-end command that moves one chosen Window to another workspace only through a capability the running COSMIC compositor both advertises and honors. This resolves the known protocol mismatch before any drag UI depends on it.

**Blocked by:** 01 — Prove a two-Window COSMIC switch.

**Status:** ready-for-agent

- [ ] Runtime probing records the advertised management protocol version and workspace-move capabilities on the packaged compositor.
- [ ] The client never issues a request whose matching capability is not advertised.
- [ ] A supported request moves one test Window exactly once and reports the resulting workspace membership.
- [ ] An ignored or rejected request leaves the Window in its original workspace and reports a clear failed capability.
- [ ] If the advertisement/handler mismatch reproduces, a minimal first-party-source-backed upstream report or patch is prepared and tracked.
- [ ] This ticket remains incomplete until the installed compositor exposes a verified advertised-and-honored move path.
- [ ] The probe covers both spanning and separate-display workspace configurations where the machine supports them.

