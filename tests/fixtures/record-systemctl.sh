#!/bin/sh
set -eu

printf '%s\n' "$*" >> "$COSMIC_WINDOW_SWITCHER_TEST_SYSTEMCTL_LOG"

if [ "$2" = "enable" ] && [ -n "${COSMIC_WINDOW_SWITCHER_TEST_ENABLE_BARRIER:-}" ]; then
    : > "$COSMIC_WINDOW_SWITCHER_TEST_ENABLE_BARRIER.reached"
    while [ ! -e "$COSMIC_WINDOW_SWITCHER_TEST_ENABLE_BARRIER.release" ]; do
        sleep 0.01
    done
fi

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
