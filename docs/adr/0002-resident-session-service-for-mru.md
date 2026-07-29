# Track MRU order in a resident session service

The package will install a lightweight per-user service that starts with the COSMIC session and records Window activation changes. COSMIC exposes current activation state and subsequent events, but not historical MRU Order, so a process launched only when `Alt+Tab` is pressed cannot satisfy the ordering invariant; the resident process also removes overlay startup latency.

The idle service tracks metadata only. Live Thumbnail capture begins when a Switching Session opens and stops when it ends. After a mid-session restart, the service enters MRU Warm-up: it places the currently focused Window first, preserves deterministic discovery order for unknown survivors, and rebuilds accurate MRU history from later focus events. It does not claim that discovery order is historical MRU.

The service is the sole owner of the compositor connection and exposes its control surface through the user-session D-Bus, as specified in ADR 0006.
