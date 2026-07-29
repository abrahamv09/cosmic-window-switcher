# 09 — Stay responsive with GPU-native capture

**What to build:** An automatic high-performance capture path that prefers DMA-BUF when the entire compositor-to-renderer contract is compatible and falls back to proven SHM behavior otherwise. Under load, input and the selected preview remain responsive while every other visible item receives fair updates.

**Blocked by:** 05 — Show Live Thumbnails through shared memory; 08 — Configure an accessible bilingual experience.

**Status:** ready-for-agent

- [ ] DMA-BUF is selected only when device, formats, modifiers, allocation, synchronization, import, and release behavior are mutually compatible.
- [ ] Any incompatible or failed DMA-BUF negotiation falls back to SHM automatically without requiring a user preference.
- [ ] Diagnostics report the active Capture Backend and fallback reason without Window titles or pixels.
- [ ] Scheduling prioritizes keyboard and pointer input, then the selected thumbnail, then fair round-robin work for other visible items.
- [ ] The user-selected Refresh Ceiling is treated as a maximum for changed content, not a guaranteed duplicate-frame rate.
- [ ] The idle service performs no capture and has effectively zero sustained capture CPU use.
- [ ] On the development baseline, selection responds within one display frame and the visible overlay is ready within 50 ms after its configured delay.
- [ ] Ten visible Windows remain smooth at the default ceiling without input stalls, and overload recovers automatically.

