# Negotiate COSMIC capabilities at runtime

The Debian package will declare the minimum COSMIC package versions used by the supported build, while the Switcher Service determines runtime compatibility from its Capability Contract rather than requiring one exact desktop release.

At startup and before Session Readiness, the service verifies the required Wayland protocol versions and window-management, capture, keyboard, workspace, and rendering capabilities. Compatible newer COSMIC releases remain usable; optional unknown features are ignored until deliberately supported.

If a required capability is absent or incompatible, the service does not reveal its overlay. Invocation delegates to the stock COSMIC switcher, while `status` and `doctor` identify the failed capability and detected protocol version.

ADR-0012 narrows what is required: per-Window workspace and output membership
is required for Visible Workspaces, but not for the default All Workspaces
scope. Missing membership remains visible in diagnostics without blocking that
scope.
