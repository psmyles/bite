#!/usr/bin/env bash
#
# Builds the signed, notarized macOS .dmg (arm64).
#
#   scripts/build-mac.sh [--skip-magick] [--skip-tests] [--skip-notarize]
#
# Steps: preflight checks -> vendor ImageMagick -> renderer/main/CLI builds ->
# electron-builder (sign + notarize + staple) -> verify the result.
#
# Credentials. The signing identity is picked up from the keychain automatically
# when exactly one "Developer ID Application" certificate is installed; set
# CSC_NAME to choose among several. The certificate has to be in a keychain even
# on CI (import the .p12 first): the ImageMagick bundle is signed by
# bundle-magick-mac.sh, outside electron-builder, and must carry the same team
# as the app or the hardened runtime will refuse to load it.
#
# Notarization needs one of these three, in the environment or in a .env.mac
# file next to this repo (gitignored, sourced automatically):
#
#   1. APPLE_KEYCHAIN_PROFILE            - a profile stored once with
#                                          `xcrun notarytool store-credentials`;
#                                          no secrets in the environment
#   2. APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD
#                                        - app-specific password from
#                                          appleid.apple.com
#   3. APPLE_API_KEY + APPLE_API_KEY_ID + APPLE_API_ISSUER
#                                        - App Store Connect API key
#
# APPLE_TEAM_ID is derived from the signing identity when not set.
#
# Pass --skip-notarize to produce a signed but un-notarized dmg for local
# testing. Such a dmg still warns on other people's machines - never ship one.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SKIP_MAGICK=0
SKIP_TESTS=0
SKIP_NOTARIZE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-magick) SKIP_MAGICK=1; shift ;;
    --skip-tests) SKIP_TESTS=1; shift ;;
    --skip-notarize) SKIP_NOTARIZE=1; shift ;;
    -h|--help) sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

step() { echo; echo "==> $*"; }
fail() { echo "error: $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Preflight - fail before a 10-minute build, not during it
# ---------------------------------------------------------------------------

step "Preflight"

[[ "$(uname -s)" == "Darwin" ]] || fail "macOS only."
[[ "$(uname -m)" == "arm64" ]] || fail "Apple Silicon only - the bundled ImageMagick is arm64."

# Local credentials, kept out of the repo.
if [[ -f "$ROOT/.env.mac" ]]; then
  echo "Reading credentials from .env.mac"
  set -a; . "$ROOT/.env.mac"; set +a
fi

# Signing identity: use CSC_NAME if given, otherwise the keychain's sole
# Developer ID Application certificate.
IDENTITIES="$(security find-identity -v -p codesigning | grep 'Developer ID Application' || true)"
[[ -n "$IDENTITIES" ]] || fail "no 'Developer ID Application' certificate in the keychain. Install one from developer.apple.com, or set CSC_NAME."
if [[ -n "${CSC_NAME:-}" ]]; then
  MATCHES="$(grep -F "$CSC_NAME" <<< "$IDENTITIES" || true)"
  [[ -n "$MATCHES" ]] || fail "signing identity '$CSC_NAME' is not in the keychain."
else
  MATCHES="$IDENTITIES"
fi
[[ "$(wc -l <<< "$MATCHES")" -eq 1 ]] || fail "several Developer ID certificates match - set CSC_NAME to one of:"$'\n'"$MATCHES"

# codesign takes the full certificate name...
SIGN_IDENTITY="$(sed -E 's/.*"(.*)"/\1/' <<< "$MATCHES")"
# ...while electron-builder picks the certificate type itself and rejects a name
# that still carries the "Developer ID Application: " prefix.
CSC_NAME="${SIGN_IDENTITY#Developer ID Application: }"
export CSC_NAME

# The team id is the parenthesised suffix of the identity name.
if [[ -z "${APPLE_TEAM_ID:-}" ]]; then
  APPLE_TEAM_ID="$(sed -E 's/.*\(([A-Z0-9]+)\)$/\1/' <<< "$SIGN_IDENTITY")"
  export APPLE_TEAM_ID
fi

# Notarization credentials - electron-builder accepts any of these three sets.
if [[ "$SKIP_NOTARIZE" -eq 1 ]]; then
  NOTARIZE_ARGS=(-c.mac.notarize=false)
  echo "WARNING: --skip-notarize - the dmg will warn on other machines."
elif [[ -n "${APPLE_KEYCHAIN_PROFILE:-}" ]]; then
  NOTARIZE_ARGS=()
  xcrun notarytool history --keychain-profile "$APPLE_KEYCHAIN_PROFILE" >/dev/null 2>&1 \
    || fail "notarytool cannot use keychain profile '$APPLE_KEYCHAIN_PROFILE'. Re-create it with: xcrun notarytool store-credentials"
elif [[ -n "${APPLE_ID:-}" || -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]]; then
  NOTARIZE_ARGS=()
  [[ -n "${APPLE_ID:-}" ]] || fail "APPLE_ID is set empty - it is required alongside APPLE_APP_SPECIFIC_PASSWORD."
  [[ -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]] || fail "APPLE_APP_SPECIFIC_PASSWORD is not set (create one at appleid.apple.com)."
elif [[ -n "${APPLE_API_KEY:-}" || -n "${APPLE_API_KEY_ID:-}" || -n "${APPLE_API_ISSUER:-}" ]]; then
  NOTARIZE_ARGS=()
  for var in APPLE_API_KEY APPLE_API_KEY_ID APPLE_API_ISSUER; do
    [[ -n "${!var:-}" ]] || fail "$var is not set (all three App Store Connect API key vars are required)."
  done
else
  fail "no notarization credentials. Easiest setup, once:

  xcrun notarytool store-credentials bite --apple-id <your-apple-id> \\
      --team-id $APPLE_TEAM_ID --password <app-specific-password>
  echo 'APPLE_KEYCHAIN_PROFILE=bite' >> .env.mac

(app-specific password: appleid.apple.com -> Sign-In and Security -> App-Specific
Passwords). Alternatively export APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD, or the
APPLE_API_* key trio, or pass --skip-notarize for a local build."
fi

VERSION="$(node -p "require('./package.json').version")"
echo "Bite $VERSION, identity: $SIGN_IDENTITY, team: $APPLE_TEAM_ID"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

if [[ "$SKIP_MAGICK" -eq 1 ]]; then
  [[ -x "$ROOT/resources/mac/magick/bin/magick" ]] || fail "--skip-magick, but resources/mac/magick is not built."
  step "Skipping ImageMagick bundle (--skip-magick)"
else
  step "Vendoring ImageMagick"
  "$ROOT/scripts/bundle-magick-mac.sh" --identity "$SIGN_IDENTITY"
fi

if [[ "$SKIP_TESTS" -eq 0 ]]; then
  step "Tests"
  npm test
fi

step "Typecheck"
npx tsc

step "Renderer + main build"
npx vite build

step "CLI build"
npx vite build --config vite.cli.config.mts
npx pkg dist-cli/cli-bundle.js --target node20-macos-arm64 --output dist-cli/cli-bundle-macos
# pkg's output is unsigned and gets stapled into the .app, so give it the same
# identity as everything else before electron-builder picks it up.
codesign --force --sign "$SIGN_IDENTITY" --options runtime --timestamp dist-cli/cli-bundle-macos

step "Packaging (sign + notarize + staple)"
# ${a[@]+"${a[@]}"} rather than "${a[@]}": under `set -u`, bash 3.2 - the bash
# macOS ships - treats an empty array expansion as an unbound variable.
npx electron-builder --mac --arm64 ${NOTARIZE_ARGS[@]+"${NOTARIZE_ARGS[@]}"}

# ---------------------------------------------------------------------------
# Verify what came out
# ---------------------------------------------------------------------------

step "Verifying"

DMG="$(ls -t "release/$VERSION"/*.dmg 2>/dev/null | head -1)" || true
[[ -n "${DMG:-}" ]] || fail "no .dmg in release/$VERSION."

APP="release/$VERSION/mac-arm64/Bite.app"
[[ -d "$APP" ]] || APP="$(find "release/$VERSION" -maxdepth 2 -name 'Bite.app' -print -quit)"

if [[ -d "$APP" ]]; then
  codesign --verify --deep --strict --verbose=2 "$APP"
  spctl --assess --type execute --verbose=2 "$APP"
  # The bundled magick has to run from inside the signed, hardened app bundle.
  "$APP/Contents/Resources/magick/bin/magick" --version | head -1
fi

if [[ "$SKIP_NOTARIZE" -eq 0 ]]; then
  xcrun stapler validate "$DMG"
fi

echo
echo "==> $DMG ($(du -h "$DMG" | cut -f1))"
