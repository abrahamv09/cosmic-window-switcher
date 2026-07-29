# Reveal only after atomic session readiness

The Switcher Grid will not become visible until Session Readiness is established. The Switcher Service must confirm the required compositor protocols, temporary keyboard focus, Window-control capability, and rendering resources before committing to the custom Switching Session.

The service may promptly map an initially transparent exclusive-keyboard layer surface so it can observe modifier release while readiness work completes. This surface contains no partial grid or thumbnail content. If the hold modifier is released before visual reveal, the quick-switch path commits without flashing the overlay.

If readiness cannot be established, the service destroys any partial surface and delegates the Invocation Request in the same direction to COSMIC's stock switcher. This avoids swallowing keyboard input or presenting an overlay that cannot activate or cancel reliably.

A capture failure isolated to one Window does not invalidate Session Readiness. That Switcher Item enters the established degraded icon-and-title state while the rest of the session continues.
