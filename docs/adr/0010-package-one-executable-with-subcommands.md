# Package one executable with subcommands

Version 1 will install one `cosmic-window-switcher` executable with the stable Command Surface `service`, `invoke next|previous`, `settings`, `enable|disable`, and `status|doctor`.

Each command runs in the process mode appropriate to its responsibility. The resident `service` owns the compositor connection and overlay, `invoke` sends D-Bus requests, `settings` presents configuration, lifecycle commands manage only app-owned integration, and diagnostic commands inspect state without mutation unless explicitly requested.

The Switcher Service already contains the rendering dependencies required by the overlay, so separate binaries would save little resident memory while increasing packaging complexity and the risk of component version skew. Internal modules remain separable even though distribution uses one executable.
