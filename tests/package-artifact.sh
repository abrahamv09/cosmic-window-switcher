#!/bin/bash

set -euo pipefail

package=${1:?usage: tests/package-artifact.sh PACKAGE.deb}

fail() {
    echo "package contract failed: $*" >&2
    exit 1
}

field() {
    dpkg-deb --field "$package" "$1"
}

contains() {
    local haystack=$1
    local needle=$2
    [[ $haystack == *"$needle"* ]] || fail "expected '$needle' in '$haystack'"
}

[[ -f $package ]] || fail "package does not exist: $package"
[[ $(field Package) == cosmic-window-switcher ]] || fail "unexpected package name"
[[ $(field Version) == 0.1.0-1 ]] || fail "unexpected package version"
[[ $(field Architecture) == amd64 ]] || fail "release package must be amd64"

depends=$(field Depends)
contains "$depends" "cosmic-comp (>= 0.1~1785277801~24.04~ffeda33)"
contains "$depends" "cosmic-launcher (>= 1.0.12~1785249651~24.04~8799503)"
contains "$depends" "cosmic-settings (>= 1.0.12~1785277759~24.04~7287257)"
contains "$depends" "dbus-user-session"
contains "$depends" "libxkbcommon0"
contains "$depends" "systemd"

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
dpkg-deb --extract "$package" "$work/root"
dpkg-deb --control "$package" "$work/control"

required_paths=(
    usr/bin/cosmic-window-switcher
    usr/lib/systemd/user/cosmic-window-switcher.service
    usr/share/dbus-1/services/io.github.abrahamv09.CosmicWindowSwitcher.service
    usr/share/applications/io.github.abrahamv09.CosmicWindowSwitcher.desktop
    usr/share/icons/hicolor/scalable/apps/io.github.abrahamv09.CosmicWindowSwitcher.svg
    usr/share/metainfo/io.github.abrahamv09.CosmicWindowSwitcher.metainfo.xml
    usr/share/cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/card_size
    usr/share/cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/dimming
    usr/share/cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/refresh_ceiling
    usr/share/cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/animations_enabled
    usr/share/cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/reveal_delay
    usr/share/cosmic-window-switcher/i18n/en/cosmic-window-switcher.ftl
    usr/share/cosmic-window-switcher/i18n/es/cosmic-window-switcher.ftl
    usr/share/doc/cosmic-window-switcher/copyright
    usr/lib/cosmic-window-switcher/uninstall-users
)

for path in "${required_paths[@]}"; do
    [[ -e $work/root/$path ]] || fail "missing installed path: /$path"
done

for document in README.md install-and-recovery.md release-validation.md; do
    plain="$work/root/usr/share/doc/cosmic-window-switcher/$document"
    [[ -e $plain || -e $plain.gz ]] || fail "missing installed document: $document"
done

[[ ! -e $work/control/conffiles ]] || fail "package defaults must not become user conffiles"

if [[ -e $work/control/postinst ]]; then
    if rg -n 'systemctl[^\n]*(enable|start)|deb-systemd-helper[^\n]*enable' "$work/control/postinst"; then
        fail "package installation must not enable or start the integration"
    fi
fi

mkdir -p \
    "$work/homes/alice/.local/state/cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1" \
    "$work/bin" \
    "$work/dpkg-root/var/lib/dpkg"
printf 'Enabled((next: None, previous: None))\n' \
    >"$work/homes/alice/.local/state/cosmic/io.github.abrahamv09.CosmicWindowSwitcher/v1/integration"
: >"$work/dpkg-root/var/lib/dpkg/status"

cat >"$work/bin/getent" <<EOF
#!/bin/sh
printf '%s\n' 'alice:x:1000:1000:Alice:$work/homes/alice:/bin/bash'
EOF
cat >"$work/bin/runuser" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >>'$work/runuser.log'
EOF
chmod +x "$work/bin/getent" "$work/bin/runuser"

dpkg_options=(
    --root="$work/dpkg-root"
    --log="$work/dpkg.log"
    --force-not-root,script-chrootless,depends
)

COSMIC_WINDOW_SWITCHER_GETENT="$work/bin/getent" \
COSMIC_WINDOW_SWITCHER_RUNUSER="$work/bin/runuser" \
COSMIC_WINDOW_SWITCHER_BINARY=/usr/bin/cosmic-window-switcher \
    dpkg "${dpkg_options[@]}" --install "$package"
[[ -x $work/dpkg-root/usr/bin/cosmic-window-switcher ]] || fail "isolated install omitted the executable"
[[ ! -e $work/runuser.log ]] || fail "clean installation changed per-user integration"

COSMIC_WINDOW_SWITCHER_GETENT="$work/bin/getent" \
COSMIC_WINDOW_SWITCHER_RUNUSER="$work/bin/runuser" \
COSMIC_WINDOW_SWITCHER_BINARY=/usr/bin/cosmic-window-switcher \
    dpkg "${dpkg_options[@]}" --install "$package"
[[ ! -e $work/runuser.log ]] || fail "package upgrade disabled the integration"

COSMIC_WINDOW_SWITCHER_GETENT="$work/bin/getent" \
COSMIC_WINDOW_SWITCHER_RUNUSER="$work/bin/runuser" \
COSMIC_WINDOW_SWITCHER_BINARY=/usr/bin/cosmic-window-switcher \
    dpkg "${dpkg_options[@]}" --remove cosmic-window-switcher

cleanup_call=$(cat "$work/runuser.log")
contains "$cleanup_call" "alice"
contains "$cleanup_call" "HOME=$work/homes/alice"
contains "$cleanup_call" "/usr/bin/cosmic-window-switcher disable --uninstall"
[[ ! -e $work/dpkg-root/usr/bin/cosmic-window-switcher ]] || fail "removal retained the executable"

before_purge=$(wc -l <"$work/runuser.log")
COSMIC_WINDOW_SWITCHER_GETENT="$work/bin/getent" \
COSMIC_WINDOW_SWITCHER_RUNUSER="$work/bin/runuser" \
COSMIC_WINDOW_SWITCHER_BINARY=/usr/bin/cosmic-window-switcher \
    dpkg "${dpkg_options[@]}" --purge cosmic-window-switcher
after_purge=$(wc -l <"$work/runuser.log")
[[ $before_purge -eq $after_purge ]] || fail "purge repeated or broadened user cleanup"

cat >"$work/bin/runuser-fails" <<'EOF'
#!/bin/sh
exit 1
EOF
chmod +x "$work/bin/runuser-fails"
COSMIC_WINDOW_SWITCHER_GETENT="$work/bin/getent" \
COSMIC_WINDOW_SWITCHER_RUNUSER="$work/bin/runuser-fails" \
COSMIC_WINDOW_SWITCHER_BINARY=/usr/bin/cosmic-window-switcher \
    dpkg "${dpkg_options[@]}" --install "$package"
if COSMIC_WINDOW_SWITCHER_GETENT="$work/bin/getent" \
    COSMIC_WINDOW_SWITCHER_RUNUSER="$work/bin/runuser-fails" \
    COSMIC_WINDOW_SWITCHER_BINARY=/usr/bin/cosmic-window-switcher \
    dpkg "${dpkg_options[@]}" --remove cosmic-window-switcher
then
    fail "removal succeeded after shortcut restoration failed"
fi
[[ -x $work/dpkg-root/usr/bin/cosmic-window-switcher ]] || \
    fail "failed removal did not retain the fallback-capable executable"

echo "package artifact contract passed: $package"
