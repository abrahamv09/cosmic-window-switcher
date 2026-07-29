# Use the user-session D-Bus for service control

Shortcut commands and the settings interface will communicate with the resident Switcher Service through a versioned interface on the user-session D-Bus. The Switcher Service remains the single owner of the COSMIC compositor connection, MRU Order, thumbnail capture, and active Switching Session.

The packaged invocation commands send small forward or reverse Invocation Requests instead of opening their own compositor connections. D-Bus supplies per-user isolation, service discovery, single-instance ownership, and standard diagnostics without a custom Unix-socket protocol. Its message overhead is negligible because the Switcher Service is already resident and the messages never carry Live Thumbnail pixels.

If the service is absent, invocation may request its D-Bus activation or restart it once according to the established recovery policy. If recovery fails, the shortcut command delegates to the stock COSMIC switcher.
