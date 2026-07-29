# Never persist Window thumbnails

Live Thumbnail frames will remain in GPU or process memory and be released when no longer needed; Window contents will never be written to disk. This gives up persistent thumbnail caching in favor of privacy, respects compositor-denied or protected content, and uses the application icon and Window title when capture is unavailable.

Screen lock, suspend, user switch, and COSMIC session shutdown are hard privacy boundaries. Any of these events cancels an active Switching Session without activating a Window, destroys its surfaces and capture buffers, and pauses MRU tracking. Unlock or resume starts from current compositor state and never restores the old overlay.

The application will collect no telemetry and upload no crash reports. Local journal diagnostics exclude thumbnail pixels and Window titles by default; users may enable temporary verbose logging when preparing an issue report.
