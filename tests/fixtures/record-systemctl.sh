#!/bin/sh
set -eu

printf '%s\n' "$*" >> "$COSMIC_WINDOW_SWITCHER_TEST_SYSTEMCTL_LOG"

