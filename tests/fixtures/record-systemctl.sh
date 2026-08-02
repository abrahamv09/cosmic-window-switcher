#!/bin/sh
set -eu

printf '%s\n' "$*" >> "$COSMIC_WINDOW_SWITCHER_TEST_SYSTEMCTL_LOG"

if [ -n "${COSMIC_WINDOW_SWITCHER_TEST_SERVICE_STATE:-}" ]; then
    case "$2" in
        enable)
            printf 'enabled\n' > "$COSMIC_WINDOW_SWITCHER_TEST_SERVICE_STATE"
            ;;
        start)
            printf 'active\n' > "$COSMIC_WINDOW_SWITCHER_TEST_SERVICE_STATE"
            ;;
        disable)
            printf 'inactive\n' > "$COSMIC_WINDOW_SWITCHER_TEST_SERVICE_STATE"
            ;;
    esac
fi
