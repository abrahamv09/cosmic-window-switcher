# Select the capture backend automatically

The Switcher Service will prefer GPU-native DMA-BUF transport for Live Thumbnail frames and automatically fall back to CPU-backed shared memory when the compositor does not offer DMA-BUF or the renderer cannot import it.

Capture Backend selection is capability-driven and is not exposed as a Switcher Preference. Users still control the thumbnail refresh ceiling, while diagnostics report the active backend and fallback reason. This avoids asking users to understand driver-specific buffer behavior while retaining a compatibility path for different GPUs and virtualized environments.

Both backends keep frames only in process, GPU, or compositor memory for the duration required by the Switching Session. Neither backend permits persistence to disk.
