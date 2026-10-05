#!/usr/bin/env bash
#
# Builds the signed, notarized and stapled macOS .dmg (arm64) - the one to ship.
#
#   packaging/build-mac-release.sh [--skip-build] [--skip-magick] [--skip-icon]
#                                  [--sign-id <identity>] [--out <dir>]
#
# Everything packaging/build-mac-dmg.sh does, plus the notary service: the .app is notarized and
# stapled, then wrapped in a .dmg which is itself signed, notarized and stapled.
#
# Both tickets matter and they are not the same ticket. The one on the .app is what lets it
# launch on a Mac that is offline or behind a filter once it has been dragged to Applications;
# without it Gatekeeper has to reach Apple on first launch and the failure reads as "damaged and
# can't be opened" rather than "no network". The one on the .dmg is what the download itself is
# checked against. So the app is notarized *before* it goes into the image - stapling rewrites
# the bundle, and a .dmg built first would carry the unstapled copy.
#
# Credentials: one of APPLE_KEYCHAIN_PROFILE, APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD, or the
# APPLE_API_KEY trio, in the environment or in a gitignored .env.mac beside the repo. They are
# checked during preflight, so a missing one fails in seconds rather than after the build. See
# packaging/mac-common.sh for the one-time setup.
#
# Signing and notarizing are the long steps and neither says much on its own, so both are wrapped
# in a heartbeat that prints the elapsed time every 30s - a build that is working looks different
# from one that has hung.
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
# Before the build, not after it: a credential that is missing or expired should cost seconds.
mac_notary_credentials
mac_sync_version
mac_build_inputs
mac_build_binaries
mac_assemble_app
mac_check_paths
mac_sign_app
mac_smoke_magick

# ---------------------------------------------------------------------------------------------
# Notarize the app, then build the image around the stapled copy
# ---------------------------------------------------------------------------------------------

say "Notarizing $PRODUCT.app"
zip_path="$(mktemp -d)/$PRODUCT.zip"
# `ditto`, not `zip(1)`: only ditto preserves the bundle's symlinks and extended attributes, and
# notarytool rejects an archive that has lost them.
ditto -c -k --keepParent "$APP" "$zip_path"
with_heartbeat "waiting on the notary service" mac_notarize "$zip_path"
rm -rf "$(dirname "$zip_path")"
xcrun stapler staple "$APP" || die "could not staple the app."
xcrun stapler validate "$APP" || die "the stapled app does not validate."

mac_make_dmg

say "Notarizing $(basename "$DMG")"
with_heartbeat "waiting on the notary service" mac_notarize "$DMG"
xcrun stapler staple "$DMG" || die "could not staple the disk image."
xcrun stapler validate "$DMG" || die "the stapled disk image does not validate."

# ---------------------------------------------------------------------------------------------
# Verify what actually ships
# ---------------------------------------------------------------------------------------------

say "Gatekeeper"
# The two questions a user's Mac will actually ask. `--type open` with the primary-signature
# context is how Gatekeeper assesses a downloaded disk image; `--type execute` would assess it as
# an app and pass even when the image itself is unsigned.
spctl --assess --type execute --verbose=2 "$APP" || die "Gatekeeper would refuse the app."
spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG" \
    || die "Gatekeeper would refuse the disk image."

say "Done"
echo "    app: $APP"
echo "    dmg: $DMG ($(du -h "$DMG" | cut -f1))"
echo "    signed, notarized and stapled - this one is shippable."
