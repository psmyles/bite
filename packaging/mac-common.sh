#!/usr/bin/env bash
#
# Shared steps for the two macOS packaging scripts. Sourced, never run:
#
#   packaging/build-mac-dmg.sh      builds and signs a .dmg
#   packaging/build-mac-release.sh  the same, then notarizes and staples it
#
# The split between them is only *when* the notary service is involved, so everything else -
# preflight, the version sync, the payload, the bundle, the signature, the disk image - lives
# here and is called by both in the order each needs. The release script has to notarize the
# .app before it goes into the .dmg (stapling rewrites the bundle), which is why these are
# functions rather than one linear script with a flag.
#
# ---------------------------------------------------------------------------------------------
# Signing
# ---------------------------------------------------------------------------------------------
#
# The identity is the keychain's sole "Developer ID Application" certificate unless --sign-id
# names one. Only that kind produces something another Mac will run: an "Apple Development"
# certificate signs fine here and is refused everywhere else, so picking one automatically would
# just move the failure to the person you sent the .dmg to.
#
# The bundled ImageMagick is *not* signed here. `scripts/bundle-magick-mac.sh` signs all ~150 of
# its Mach-Os when it builds the tree, each with its own timestamp round-trip, and doing it again
# per release would add minutes for nothing. What this does instead is verify them, and insist
# they carry the same team as the app - the hardened runtime enforces exactly that at load time,
# so a mismatch found here is a crash found on the user's machine otherwise.
#
# ---------------------------------------------------------------------------------------------
# Notarization credentials
# ---------------------------------------------------------------------------------------------
#
# One of these three, in the environment or in a gitignored `.env.mac` beside the repo, which is
# sourced automatically:
#
#   1. APPLE_KEYCHAIN_PROFILE   a profile stored once with `xcrun notarytool store-credentials`;
#                               no secrets in the environment. This is the easy one.
#   2. APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD     from appleid.apple.com
#   3. APPLE_API_KEY + APPLE_API_KEY_ID + APPLE_API_ISSUER   an App Store Connect key
#
# APPLE_TEAM_ID is derived from the signing identity when it is not set.

# ---------------------------------------------------------------------------------------------
# Output helpers
# ---------------------------------------------------------------------------------------------

say() { printf '\n==> %s\n' "$*"; }
note() { printf '    %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Runs a command that prints little or nothing for minutes, reporting the elapsed time every 30s
# so a stalled step is distinguishable from a working one. The command keeps this script's
# stdout, so its own output still comes through, and its exit status is this function's.
with_heartbeat() {
    local label="$1"; shift
    "$@" &
    local pid=$! elapsed=0
    while kill -0 "$pid" 2>/dev/null; do
        sleep 1
        elapsed=$((elapsed + 1))
        if (( elapsed % 30 == 0 )) && kill -0 "$pid" 2>/dev/null; then
            printf '    %s ... %dm%02ds elapsed\n' "$label" $((elapsed / 60)) $((elapsed % 60))
        fi
    done
    wait "$pid"
}

# ---------------------------------------------------------------------------------------------
# Shared option parsing
# ---------------------------------------------------------------------------------------------

SKIP_BUILD=0
SKIP_MAGICK=0
SKIP_ICON=0
SIGN_ID=""
OUT_DIR=""

# Prints the header comment of the calling script as its --help.
mac_usage() { sed -n '2,/^$/p' "$1" | sed 's/^# \{0,1\}//'; }

mac_parse_args() {
    local script="$1"; shift
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-build)  SKIP_BUILD=1; shift ;;
            --skip-magick) SKIP_MAGICK=1; shift ;;
            --skip-icon)   SKIP_ICON=1; shift ;;
            --sign-id)     SIGN_ID="${2:-}"; shift 2 ;;
            --sign-id=*)   SIGN_ID="${1#*=}"; shift ;;
            --out)         OUT_DIR="${2:-}"; shift 2 ;;
            --out=*)       OUT_DIR="${1#*=}"; shift ;;
            -h|--help)     mac_usage "$script"; exit 0 ;;
            *) echo "unknown argument: $1" >&2; mac_usage "$script" >&2; exit 2 ;;
        esac
    done
}

# ---------------------------------------------------------------------------------------------
# 1. Preflight - fail before a long build, not during it
# ---------------------------------------------------------------------------------------------

# Sets: ROOT, PRODUCT, VERSION, COPYRIGHT, BUNDLE_ID, EXE_NAME, CLI_NAME, EXTENSION, HOMEPAGE,
#       SIGN_IDENTITY, APPLE_TEAM_ID, APP, DMG, OUT_DIR.
mac_preflight() {
    ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    cd "$ROOT"

    [[ "$(uname -s)" == "Darwin" ]] || die "macOS only."
    # The vendored ImageMagick is arm64-only, so a universal or x86_64 build would ship a binary
    # its own payload cannot run beside. Checked here rather than pretended around.
    [[ "$(uname -m)" == "arm64" ]] || die "Apple Silicon only - the bundled ImageMagick is arm64."

    # Local credentials, kept out of the repo.
    if [[ -f "$ROOT/.env.mac" ]]; then
        note "Reading credentials from .env.mac"
        set -a; . "$ROOT/.env.mac"; set +a
    fi

    # product.json is the one source of product identity, read by `bite-gui/build.rs` and by the
    # Windows packaging script too. `plutil` reads JSON, so this needs nothing installed.
    local json="$ROOT/product.json"
    [[ -f "$json" ]] || die "product.json not found at $json."
    local field
    for field in productName version copyright bundleId exeName cliName homepage; do
        plutil -extract "$field" raw -o - "$json" >/dev/null 2>&1 \
            || die "product.json is missing the string field \"$field\"."
    done
    PRODUCT="$(plutil -extract productName raw -o - "$json")"
    VERSION="$(plutil -extract version raw -o - "$json")"
    COPYRIGHT="$(plutil -extract copyright raw -o - "$json")"
    BUNDLE_ID="$(plutil -extract bundleId raw -o - "$json")"
    EXE_NAME="$(plutil -extract exeName raw -o - "$json")"
    CLI_NAME="$(plutil -extract cliName raw -o - "$json")"
    HOMEPAGE="$(plutil -extract homepage raw -o - "$json")"
    # ".bite" in the manifest, "bite" in a plist.
    EXTENSION="$(plutil -extract fileAssociation.extension raw -o - "$json")"
    EXTENSION="${EXTENSION#.}"

    # The signing identity. codesign wants the full certificate name.
    local identities matches
    identities="$(security find-identity -v -p codesigning | grep 'Developer ID Application' || true)"
    [[ -n "$identities" ]] || die "no 'Developer ID Application' certificate in the keychain.
  Install one from developer.apple.com, or pass --sign-id."
    if [[ -n "$SIGN_ID" ]]; then
        matches="$(grep -F "$SIGN_ID" <<< "$identities" || true)"
        [[ -n "$matches" ]] || die "signing identity '$SIGN_ID' is not in the keychain."
    else
        matches="$identities"
    fi
    [[ "$(wc -l <<< "$matches")" -eq 1 ]] \
        || die "several Developer ID certificates match - pass --sign-id with one of:"$'\n'"$matches"
    SIGN_IDENTITY="$(sed -E 's/.*"(.*)"/\1/' <<< "$matches")"
    # The team id is the parenthesised suffix of the identity name.
    if [[ -z "${APPLE_TEAM_ID:-}" ]]; then
        APPLE_TEAM_ID="$(sed -E 's/.*\(([A-Z0-9]+)\)$/\1/' <<< "$SIGN_IDENTITY")"
        export APPLE_TEAM_ID
    fi

    OUT_DIR="${OUT_DIR:-$ROOT/dist}"
    APP="$ROOT/target/mac/$PRODUCT.app"
    DMG="$OUT_DIR/$PRODUCT-Mac-$VERSION-arm64.dmg"

    note "$PRODUCT $VERSION ($BUNDLE_ID)"
    note "identity: $SIGN_IDENTITY"
    note "team:     $APPLE_TEAM_ID"
}

# ---------------------------------------------------------------------------------------------
# 2. Sync the Cargo workspace version to product.json
# ---------------------------------------------------------------------------------------------
# The twin of build-windows-installer.ps1's step 2, and here for the same reason: product.json is
# the single source of the version, but nothing in a `cargo build` reads it into a manifest, so
# `[workspace.package] version` drifts silently until something that *does* read it disagrees
# with the app. The only line-anchored `version = "..."` in the root manifest belongs to
# [workspace.package] - dependency versions live inside inline tables - so the anchor is unique.

mac_sync_version() {
    say "Syncing Cargo.toml [workspace.package] version to $VERSION"
    local manifest="$ROOT/Cargo.toml" current
    current="$(sed -n -E 's/^version[[:space:]]*=[[:space:]]*"([^"]*)".*/\1/p' "$manifest" | head -1)"
    [[ -n "$current" ]] || die "no [workspace.package] version found in $manifest."
    if [[ "$current" == "$VERSION" ]]; then
        note "already in sync."
    else
        sed -i '' -E "s/^version[[:space:]]*=[[:space:]]*\"[^\"]*\"/version = \"$VERSION\"/" "$manifest"
        note "updated: $current -> $VERSION."
        [[ "$SKIP_BUILD" -eq 0 ]] || note "note: --skip-build, so target/release still holds a $current build."
    fi
    # Run unconditionally: the manifest can be in sync while the committed lock is not.
    # `--workspace` re-resolves the members only, so every registry dependency stays pinned.
    cargo update --manifest-path "$manifest" --workspace --quiet \
        || die "cargo update failed - Cargo.lock is not synced to $VERSION."
}

# ---------------------------------------------------------------------------------------------
# 3. The two payload inputs, neither of which is in git
# ---------------------------------------------------------------------------------------------

mac_build_inputs() {
    local magick="$ROOT/resources/mac/magick/bin/magick"
    if [[ "$SKIP_MAGICK" -eq 1 ]]; then
        [[ -x "$magick" ]] || die "--skip-magick, but $magick is not built. Run scripts/bundle-magick-mac.sh."
        say "Skipping the ImageMagick bundle (--skip-magick)"
    else
        say "Vendoring ImageMagick"
        "$ROOT/scripts/bundle-magick-mac.sh" --identity "$SIGN_IDENTITY" \
            || die "scripts/bundle-magick-mac.sh failed."
    fi

    local icns="$ROOT/build/icons/mac/icon.icns" car="$ROOT/build/icons/mac/Assets.car"
    if [[ "$SKIP_ICON" -eq 1 ]]; then
        [[ -f "$icns" && -f "$car" ]] \
            || die "--skip-icon, but build/icons/mac is not built. Run scripts/build-icon-mac.sh."
        say "Skipping the icon compile (--skip-icon)"
    else
        say "Compiling the icon"
        "$ROOT/scripts/build-icon-mac.sh" || die "scripts/build-icon-mac.sh failed."
    fi
}

# ---------------------------------------------------------------------------------------------
# 4. Build
# ---------------------------------------------------------------------------------------------

mac_build_binaries() {
    if [[ "$SKIP_BUILD" -eq 1 ]]; then
        say "Skipping the cargo build (--skip-build)"
    else
        say "Building the release binaries"
        cargo build --manifest-path "$ROOT/Cargo.toml" --release -p bite-gui -p bite-cli \
            || die "cargo build failed."
    fi

    local binary
    for binary in "$ROOT/target/release/$EXE_NAME" "$ROOT/target/release/$CLI_NAME"; do
        [[ -x "$binary" ]] || die "release binary not found at $binary. Drop --skip-build?"
        lipo -archs "$binary" | grep -qw arm64 \
            || die "$binary is '$(lipo -archs "$binary")', not arm64."
    done
}

# ---------------------------------------------------------------------------------------------
# 5. Assemble the bundle
# ---------------------------------------------------------------------------------------------
#
# Layout, which the binaries' own lookups decide rather than convention alone:
#
#   Contents/MacOS/bite-gui            the editor
#   Contents/MacOS/bite                the CLI, beside it
#   Contents/Resources/magick/bin/...  Magick::discover probes ../Resources/magick/bin/magick
#   Contents/Resources/node-definitions, format-definitions
#                                      app::definitions_root and bite-cli's load_registry both
#                                      probe ../Resources
#   Contents/Resources/Assets.car      the layered icon macOS 26+ draws, named by CFBundleIconName
#   Contents/Resources/icon.icns       the flattened fallback, and the icon Finder gives the .dmg

mac_assemble_app() {
    say "Assembling $PRODUCT.app"
    rm -rf "$APP"
    mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

    cp "$ROOT/target/release/$EXE_NAME" "$APP/Contents/MacOS/$EXE_NAME"
    cp "$ROOT/target/release/$CLI_NAME" "$APP/Contents/MacOS/$CLI_NAME"

    # `ditto` rather than `cp -R`: the magick tree is signed, and `cp` can drop the extended
    # attributes a signature lives in.
    ditto "$ROOT/resources/mac/magick" "$APP/Contents/Resources/magick"
    cp "$ROOT/build/icons/mac/icon.icns" "$APP/Contents/Resources/icon.icns"
    cp "$ROOT/build/icons/mac/Assets.car" "$APP/Contents/Resources/Assets.car"

    # Node and format definitions ship as plain JSON so users can add their own.
    local definitions count
    for definitions in node-definitions format-definitions; do
        mkdir -p "$APP/Contents/Resources/$definitions"
        cp "$ROOT/$definitions"/*.json "$APP/Contents/Resources/$definitions/" 2>/dev/null || true
        count="$(ls -1 "$APP/Contents/Resources/$definitions"/*.json 2>/dev/null | wc -l | tr -d ' ')"
        [[ "$count" -gt 0 ]] || die "no definitions in $ROOT/$definitions - the app would have no nodes."
        note "$definitions: $count definitions."
    done

    # The ImageMagick license requires its notice to travel with the binary, so a bundle built
    # without it is a compliance bug rather than a cosmetic omission.
    local notice
    for notice in LICENSE THIRD_PARTY_LICENSES; do
        [[ -f "$ROOT/$notice" ]] || die "missing license file the bundle must ship: $notice"
        cp "$ROOT/$notice" "$APP/Contents/Resources/$notice"
    done

    # The user-facing documents only. docs/node-authoring-guide.md is deliberately not shipped:
    # it still describes the v1 definition format and is queued for a rewrite.
    mkdir -p "$APP/Contents/Resources/docs"
    cp "$ROOT/docs/getting-started.md" "$ROOT/docs/expression-language.md" \
        "$APP/Contents/Resources/docs/"
    # Example workflows, so a new install has something to open.
    if compgen -G "$ROOT/examples/*.bite" >/dev/null; then
        mkdir -p "$APP/Contents/Resources/examples"
        cp "$ROOT"/examples/*.bite "$APP/Contents/Resources/examples/"
    fi

    # The four-byte type/creator file. Vestigial, but its absence still confuses some tools.
    printf 'APPL????' > "$APP/Contents/PkgInfo"

    local type_name
    type_name="$(plutil -extract fileAssociation.typeName raw -o - "$ROOT/product.json")"
    cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>            <string>$EXE_NAME</string>
    <key>CFBundleIdentifier</key>            <string>$BUNDLE_ID</string>
    <key>CFBundleName</key>                  <string>$PRODUCT</string>
    <key>CFBundleDisplayName</key>           <string>$PRODUCT</string>
    <key>CFBundlePackageType</key>           <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key> <string>6.0</string>
    <key>CFBundleShortVersionString</key>    <string>$VERSION</string>
    <key>CFBundleVersion</key>               <string>$VERSION</string>
    <key>NSHumanReadableCopyright</key>      <string>$COPYRIGHT</string>
    <key>LSApplicationCategoryType</key>     <string>public.app-category.graphics-design</string>
    <!-- The flattened icon, for the .dmg and anything reading CFBundleIconFile... -->
    <key>CFBundleIconFile</key>              <string>icon</string>
    <!-- ...and the layered catalogue macOS 26+ draws instead. The name is the one
         scripts/build-icon-mac.sh compiled Assets.car under, so the two move together. -->
    <key>CFBundleIconName</key>              <string>bite</string>
    <!-- arm64 only, and the vendored ImageMagick's dylibs were built against this floor. -->
    <key>LSMinimumSystemVersion</key>        <string>12.0</string>
    <!-- Not optional: without it macOS runs the window through 1x scaling and every pixel is
         blurry on a Retina display. -->
    <key>NSHighResolutionCapable</key>       <true/>
    <!-- The workflow file, which Finder hands over as an Apple event rather than an argument;
         see crates/bite-gui/src/openfiles.rs. Owner rank, not Alternate: the editor defines the
         type, so it is the one application that should open it by default. -->
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>     <string>$type_name</string>
            <key>CFBundleTypeRole</key>     <string>Editor</string>
            <key>LSHandlerRank</key>        <string>Owner</string>
            <key>CFBundleTypeIconFile</key> <string>icon</string>
            <key>LSItemContentTypes</key>
            <array><string>$BUNDLE_ID.workflow</string></array>
        </dict>
    </array>
    <key>UTExportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key>  <string>$BUNDLE_ID.workflow</string>
            <key>UTTypeDescription</key> <string>$type_name</string>
            <key>UTTypeIconFile</key>    <string>icon</string>
            <!-- A workflow is JSON, so anything that reads JSON can be offered it too. -->
            <key>UTTypeConformsTo</key>
            <array>
                <string>public.json</string>
                <string>public.data</string>
            </array>
            <key>UTTypeTagSpecification</key>
            <dict>
                <key>public.filename-extension</key>
                <array><string>$EXTENSION</string></array>
            </dict>
        </dict>
    </array>
</dict>
</plist>
PLIST
    plutil -lint "$APP/Contents/Info.plist" >/dev/null || die "the generated Info.plist is malformed."
    note "$APP"
}

# ---------------------------------------------------------------------------------------------
# 6. Nothing may point at the build machine
# ---------------------------------------------------------------------------------------------
# A dylib still referencing /opt/homebrew runs perfectly here and fails on every machine that has
# no Homebrew, which is the failure this catches at build time rather than in someone's download.

mac_check_paths() {
    say "Checking for build-machine paths"
    local offenders
    offenders="$(
        find "$APP" -type f -perm +111 -print0 2>/dev/null \
            | xargs -0 -n1 otool -L 2>/dev/null \
            | grep -E '^\s+(/opt/homebrew|/usr/local)/' | sort -u || true
    )"
    if [[ -n "$offenders" ]]; then
        echo "$offenders" >&2
        die "the bundle links against the build machine's own prefixes."
    fi
    note "no /opt/homebrew or /usr/local references."
}

# ---------------------------------------------------------------------------------------------
# 7. Sign
# ---------------------------------------------------------------------------------------------
# Inside-out, and without --deep. `--deep` is deprecated, and it would re-sign the ImageMagick
# tree - throwing away ~150 already-timestamped signatures to redo them, and papering over a
# broken one rather than reporting it.
#
# No entitlements. The Electron bundle needed `allow-jit` and `allow-unsigned-executable-memory`
# for V8 and `disable-library-validation` because its frameworks were loaded across teams; none
# of that applies to a Rust binary whose only dlopens are magick's own coder modules, signed by
# the same team. Every entitlement is a hole to justify, and these have nothing to justify them.

mac_sign_app() {
    say "Signing"

    # The ImageMagick tree carries scripts/bundle-magick-mac.sh's signatures. Verify rather than
    # redo, and insist on the same team: library validation under the hardened runtime refuses a
    # dylib signed by anyone else, so a mismatch here is a launch failure on the user's machine.
    local magick="$APP/Contents/Resources/magick/bin/magick"
    [[ -x "$magick" ]] || die "the bundled magick is missing from the app."
    codesign --verify --strict "$magick" \
        || die "the bundled ImageMagick is not validly signed. Re-run scripts/bundle-magick-mac.sh."
    # Read the signature once into a variable rather than grepping the command twice. Under
    # `set -o pipefail` a `codesign -dvv | grep -q` pipeline reports *failure* on a match: grep
    # leaves as soon as it has one, codesign takes a SIGPIPE writing the rest, and pipefail
    # surfaces that. The here-strings below read a string, so there is no pipe to break.
    local signature magick_team
    signature="$(codesign -dvv "$magick" 2>&1 || true)"
    magick_team="$(sed -n 's/^TeamIdentifier=//p' <<< "$signature")"
    [[ "$magick_team" == "$APPLE_TEAM_ID" ]] \
        || die "the bundled ImageMagick is signed by team '$magick_team', not '$APPLE_TEAM_ID'.
  The hardened runtime will refuse to load it. Re-run scripts/bundle-magick-mac.sh --identity '$SIGN_IDENTITY'."
    grep -q 'flags=.*runtime' <<< "$signature" \
        || die "the bundled ImageMagick is not signed with the hardened runtime."
    note "bundled ImageMagick: team $magick_team, hardened."

    # `--options runtime` is the hardened runtime, which notarization requires; `--timestamp` is
    # the secure timestamp it also requires, and what keeps the signature valid after the
    # certificate expires.
    codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" \
        "$APP/Contents/MacOS/$CLI_NAME" || die "could not sign the CLI."
    codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP" \
        || die "could not sign $PRODUCT.app."
    codesign --verify --deep --strict --verbose=2 "$APP" || die "the signed app does not verify."

}

# ---------------------------------------------------------------------------------------------
# 7b. The bundled ImageMagick has to actually decode something
# ---------------------------------------------------------------------------------------------
# `magick --version` is not this test: it prints happily from a tree whose coder modules cannot
# be loaded at all, and a bundle in that state ships and then fails on the first image the user
# opens. Homebrew builds ImageMagick with loadable coders, so the formats live in
# lib/ImageMagick/**/*.so rather than in the binary, and finding them depends on the four
# MAGICK_* variables below - which is why this sets exactly the ones `Magick::discover` sets on
# macOS (crates/bite-imagemagick/src/lib.rs) rather than a convenient approximation.
#
# Run after signing, deliberately: the hardened runtime is what makes library validation refuse a
# dylib from another team, so this is also where a mis-signed coder module shows up. `env -i`
# leaves no Homebrew on PATH and no DYLD_* crutches - exactly a user's machine.

mac_smoke_magick() {
    say "Smoke testing the bundled ImageMagick"
    local magick="$APP/Contents/Resources/magick/bin/magick"
    local root="$APP/Contents/Resources/magick"
    local modules config_modules config_dir
    modules="$(cd "$root/lib/ImageMagick" && echo modules-*)"
    config_modules="$(cd "$root/lib/ImageMagick" && echo config-*)"
    config_dir="$(cd "$root/etc" && echo ImageMagick-*)"
    local magick_env=(
        MAGICK_HOME="$root"
        MAGICK_CONFIGURE_PATH="$root/lib/ImageMagick/$config_modules:$root/etc/$config_dir:$root/share/$config_dir"
        MAGICK_CODER_MODULE_PATH="$root/lib/ImageMagick/$modules/coders"
        MAGICK_FILTER_MODULE_PATH="$root/lib/ImageMagick/$modules/filters"
    )

    note "$(env -i PATH=/usr/bin:/bin "${magick_env[@]}" "$magick" --version | head -1)"

    # Encode and decode for real: a coder that cannot load reports "no decode delegate" here
    # rather than in someone's first workflow.
    local bytes
    bytes="$(env -i PATH=/usr/bin:/bin "${magick_env[@]}" "$magick" \
        -size 16x16 gradient:red-blue png:- 2>/dev/null | wc -c | tr -d ' ')"
    [[ "${bytes:-0}" -gt 100 ]] \
        || die "the bundled ImageMagick produced no PNG - its coder modules are not loading.
  Re-run scripts/bundle-magick-mac.sh (without --skip-magick)."

    # The formats the editor's own defaults reach for. A missing one is a warning, not a failure:
    # the bundle still works for everything else, and which coders are present is a choice
    # scripts/bundle-magick-mac.sh makes (--no-heic drops the copyleft codecs).
    local formats listed
    listed="$(env -i PATH=/usr/bin:/bin "${magick_env[@]}" "$magick" -list format 2>/dev/null || true)"
    for formats in JPEG PNG TIFF WEBP; do
        grep -qE "^ *$formats\*? " <<< "$listed" \
            || echo "warning: the bundled ImageMagick reports no $formats support" >&2
    done
    note "coders load and encode."
}

# ---------------------------------------------------------------------------------------------
# 8. The disk image
# ---------------------------------------------------------------------------------------------

mac_make_dmg() {
    say "Building the disk image"
    mkdir -p "$OUT_DIR"
    local staging
    staging="$(mktemp -d)/$PRODUCT"
    mkdir -p "$staging"
    # `ditto` again, for the reason above: `cp -R` on a signed bundle can drop extended
    # attributes and invalidate the signature.
    ditto "$APP" "$staging/$PRODUCT.app"
    # The drag-to-install target. A .dmg with only the app in it teaches people to run it from
    # the disk image, where it stays read-only and every update re-downloads.
    ln -s /Applications "$staging/Applications"
    rm -f "$DMG"
    # `hdiutil` prints a deprecation notice on macOS 26+, pointing at `diskutil image create`.
    # It is left as it is on purpose: the replacement exists on no earlier macOS, and a release
    # script that only runs on the newest one is worse than a warning. UDZO is the compressed,
    # read-only format every Mac can mount.
    hdiutil create -volname "$PRODUCT $VERSION" -srcfolder "$staging" -ov -format UDZO "$DMG" >/dev/null \
        || die "hdiutil could not create the disk image."
    rm -rf "$(dirname "$staging")"

    # The .dmg needs a signature of its own: it is what the user downloads and double-clicks, and
    # Gatekeeper checks the container before anything inside it.
    codesign --force --timestamp --sign "$SIGN_IDENTITY" "$DMG" || die "could not sign the disk image."
    codesign --verify --verbose=2 "$DMG" || die "the signed disk image does not verify."
    note "$DMG ($(du -h "$DMG" | cut -f1))"
}

# ---------------------------------------------------------------------------------------------
# 9. Notarization
# ---------------------------------------------------------------------------------------------

# Fills NOTARY_CREDS from whichever of the three credential sets is present. Called only by the
# release script, and called during *its* preflight so a missing credential fails in seconds
# rather than after a ten-minute build.
mac_notary_credentials() {
    if [[ -n "${APPLE_KEYCHAIN_PROFILE:-}" ]]; then
        NOTARY_CREDS=(--keychain-profile "$APPLE_KEYCHAIN_PROFILE")
        xcrun notarytool history "${NOTARY_CREDS[@]}" >/dev/null 2>&1 \
            || die "notarytool cannot use the keychain profile '$APPLE_KEYCHAIN_PROFILE'.
  Re-create it with: xcrun notarytool store-credentials"
        note "notarization: keychain profile '$APPLE_KEYCHAIN_PROFILE'"
    elif [[ -n "${APPLE_ID:-}" || -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]]; then
        [[ -n "${APPLE_ID:-}" ]] || die "APPLE_ID is empty - it is required alongside APPLE_APP_SPECIFIC_PASSWORD."
        [[ -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]] \
            || die "APPLE_APP_SPECIFIC_PASSWORD is not set (create one at appleid.apple.com)."
        NOTARY_CREDS=(--apple-id "$APPLE_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --team-id "$APPLE_TEAM_ID")
        note "notarization: Apple ID $APPLE_ID"
    elif [[ -n "${APPLE_API_KEY:-}" || -n "${APPLE_API_KEY_ID:-}" || -n "${APPLE_API_ISSUER:-}" ]]; then
        local var
        for var in APPLE_API_KEY APPLE_API_KEY_ID APPLE_API_ISSUER; do
            [[ -n "${!var:-}" ]] || die "$var is not set (all three App Store Connect key variables are required)."
        done
        NOTARY_CREDS=(--key "$APPLE_API_KEY" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER")
        note "notarization: App Store Connect key $APPLE_API_KEY_ID"
    else
        die "no notarization credentials. Easiest setup, once:

  xcrun notarytool store-credentials bite --apple-id <your-apple-id> \\
      --team-id $APPLE_TEAM_ID --password <app-specific-password>
  echo 'APPLE_KEYCHAIN_PROFILE=bite' >> .env.mac

(app-specific password: appleid.apple.com -> Sign-In and Security -> App-Specific Passwords.)
Alternatively export APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD, or the APPLE_API_* key trio, or use
packaging/build-mac-dmg.sh for a signed build that is not notarized."
    fi
}

# Submits `$1` and waits. On failure, prints the one command that says *why*: notarytool's exit
# status alone does not, and `set -e` would otherwise end the script before the id is visible.
mac_notarize() {
    local path="$1" log status id
    log="$(mktemp)"
    xcrun notarytool submit "$path" "${NOTARY_CREDS[@]}" --wait 2>&1 | tee "$log"
    status=${PIPESTATUS[0]}
    if [[ $status -ne 0 ]]; then
        id="$(sed -n 's/^ *id: *\([0-9a-f-]*\)$/\1/p' "$log" | head -1)"
        echo >&2
        if [[ -n "$id" ]]; then
            echo "error: notarization failed. Apple's reasons:" >&2
            echo "  xcrun notarytool log $id ${NOTARY_CREDS[*]}" >&2
        else
            echo "error: notarization failed before the upload." >&2
        fi
        rm -f "$log"
        exit 1
    fi
    rm -f "$log"
}
