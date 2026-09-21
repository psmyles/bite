#!/usr/bin/env bash
#
# Builds the signed macOS .dmg (arm64).
#
#   packaging/build-mac-dmg.sh [--skip-build] [--skip-magick] [--skip-icon]
#                              [--sign-id <identity>] [--out <dir>]
#
# Steps: preflight -> sync the version -> vendor ImageMagick -> compile the icon -> build both
# binaries -> assemble Bite.app -> sign it -> wrap it in a signed .dmg.
#
# The result is signed with your Developer ID but **not notarized**, so it opens here and on any
# Mac that has run it before, and on a stranger's Mac it needs System Settings -> Privacy &
# Security -> Open Anyway. That makes it right for testing the real packaged app and wrong for
# shipping. Use packaging/build-mac-release.sh for anything you send to someone.
#
# Options
#   --skip-build          reuse target/release as it stands
#   --skip-magick         reuse resources/mac/magick (scripts/bundle-magick-mac.sh's output)
#   --skip-icon           reuse build/icons/mac (scripts/build-icon-mac.sh's output)
#   --sign-id <identity>  codesign identity; the default is the keychain's sole
#                         "Developer ID Application" certificate
#   --out <dir>           where the .dmg goes (default: dist/)
#
# It edits one tracked file: Cargo.toml's [workspace.package] version, synced to product.json so
# the manifest cannot drift from the version the bundle reports. Everything else goes to target/
# and dist/.
set -euo pipefail

. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/mac-common.sh"
mac_parse_args "${BASH_SOURCE[0]}" "$@"

say "Preflight"
mac_preflight
mac_sync_version
mac_build_inputs
mac_build_binaries
mac_assemble_app
mac_check_paths
mac_sign_app
mac_smoke_magick
mac_make_dmg

say "Done"
echo "    app: $APP"
echo "    dmg: $DMG"
echo
echo "    SIGNED, NOT NOTARIZED. On another Mac this opens only through System Settings ->"
echo "    Privacy & Security -> Open Anyway; macOS 15 removed the Control-click bypass."
echo "    Run packaging/build-mac-release.sh for a build you can send to someone."
