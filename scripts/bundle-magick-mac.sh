#!/usr/bin/env bash
#
# Vendors a self-contained arm64 ImageMagick into resources/mac/magick/ so the
# packaged macOS app ships its own binary, the way resources/win/magick/ does on
# Windows.  Upstream publishes no relocatable macOS build, so we build the pinned
# release below from source against Homebrew's libraries, then take that install
# apart: copy `magick` plus every non-system dylib it pulls in, rewrite the
# absolute Cellar install names to @rpath, and re-sign everything
# (install_name_tool invalidates the signature, and arm64 refuses to load an
# unsigned Mach-O).
#
# Why not Homebrew's own imagemagick: its formula is configured --without-openexr,
# so it cannot read or write EXR, which the Windows build (upstream's portable
# release, OpenEXR built in) can.  imagemagick-full has OpenEXR but also links
# Ghostscript, which is AGPL.  So the configure line here is the imagemagick
# formula's with OpenEXR turned on, and the build lands in target/magick-mac/,
# reused by later runs until IM_VERSION changes or --rebuild is passed.
#
# To move to a new release, set IM_VERSION and IM_SHA256 below from the release's
# ImageMagick-<version>.7z asset on GitHub (the API lists its sha256 digest), and
# update resources/win/magick from the same release's portable-Q16-HDRI-x64.7z.
#
# The binaries look for it at resources/mac/magick/bin/magick relative to the working
# directory, which is how a `cargo run` finds it, and at ../Resources/magick/bin/magick
# relative to the executable, which is where the packaged app carries it (see
# Magick::discover in crates/bite-imagemagick/src/lib.rs). packaging/mac-common.sh runs
# this as part of either macOS build script and copies the tree into the bundle as it
# stands - it is signed here, not there.
#
# Layout produced:
#
#   resources/mac/magick/bin/magick
#   resources/mac/magick/lib/*.dylib
#   resources/mac/magick/lib/ImageMagick/modules-Q16HDRI/{coders,filters}/*.so
#   resources/mac/magick/lib/ImageMagick/config-Q16HDRI/configure.xml
#   resources/mac/magick/etc/ImageMagick-7/*.xml
#   resources/mac/magick/share/ImageMagick-7/*
#
# Usage:  scripts/bundle-magick-mac.sh [--identity "Developer ID Application: ..."] [--no-heic]
#                                     [--rebuild]
#
# Homebrew provides the libraries; the script names the `brew install` line when
# one is missing.
#
# The identity defaults to $CSC_NAME, then $APPLE_SIGNING_IDENTITY.  Pass
# --identity - for an ad-hoc signature (fine for local runs, not for a release).
#
# --no-heic drops the HEIC/AVIF coder.  That coder is the only thing that pulls
# in libheif and its codecs (x265 is GPLv2, libde265 LGPLv3), so dropping it
# takes ~20 MB off the bundle and leaves nothing copyleft in it.
#
# --rebuild compiles ImageMagick again even if target/magick-mac/ already holds
# this version.

set -euo pipefail

IM_VERSION="7.1.2-34"
IM_SHA256="a376053195cbab4618855e1a7d90f85f41b95ec2240a91e9c0fe5815d231960d"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/resources/mac/magick"
BUILD_ROOT="$ROOT/target/magick-mac"
IDENTITY="${CSC_NAME:-${APPLE_SIGNING_IDENTITY:-}}"
WITH_HEIC=1
REBUILD=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --identity) IDENTITY="${2:-}"; shift 2 ;;
    --identity=*) IDENTITY="${1#*=}"; shift ;;
    --no-heic) WITH_HEIC=0; shift ;;
    --rebuild) REBUILD=1; shift ;;
    -h|--help) sed -n '2,/^$/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[[ "$(uname -s)" == "Darwin" ]] || { echo "error: macOS only." >&2; exit 1; }
[[ "$(uname -m)" == "arm64" ]] || { echo "error: run this on an Apple Silicon Mac - the bundle is arm64-only." >&2; exit 1; }

if [[ -z "$IDENTITY" ]]; then
  echo "error: no signing identity. Set CSC_NAME to your 'Developer ID Application: ...' identity," >&2
  echo "       or pass --identity - for an unsigned local bundle." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Build ImageMagick from source, with OpenEXR
# ---------------------------------------------------------------------------

if ! command -v brew >/dev/null 2>&1; then
  echo "error: Homebrew not found. It provides the libraries ImageMagick is built against." >&2
  exit 1
fi
BREW="$(brew --prefix)"

# The imagemagick formula's dependencies, plus OpenEXR (which brings Imath).
DEPS=(pkgconf freetype jpeg-turbo libheif libpng libtiff libtool little-cms2 webp xz zstd openexr)
missing=()
for dep in "${DEPS[@]}"; do
  [[ -d "$BREW/opt/$dep" ]] || missing+=("$dep")
done
if [[ ${#missing[@]} -gt 0 ]]; then
  echo "error: missing Homebrew packages. Run: brew install ${missing[*]}" >&2
  exit 1
fi

PREFIX="$BUILD_ROOT/$IM_VERSION"
SRC_MAGICK="$PREFIX/bin/magick"

if [[ "$REBUILD" -eq 1 || ! -x "$SRC_MAGICK" ]]; then
  echo "==> Building ImageMagick $IM_VERSION with OpenEXR"
  archive="$BUILD_ROOT/ImageMagick-$IM_VERSION.7z"
  mkdir -p "$BUILD_ROOT"
  if [[ ! -f "$archive" ]] || ! shasum -a 256 "$archive" | grep -q "^$IM_SHA256 "; then
    curl -fsSL -o "$archive.part" \
      "https://github.com/ImageMagick/ImageMagick/releases/download/$IM_VERSION/ImageMagick-$IM_VERSION.7z"
    mv "$archive.part" "$archive"
  fi
  actual="$(shasum -a 256 "$archive" | cut -d' ' -f1)"
  if [[ "$actual" != "$IM_SHA256" ]]; then
    echo "error: ImageMagick-$IM_VERSION.7z has sha256 $actual, expected $IM_SHA256." >&2
    rm -f "$archive"
    exit 1
  fi

  rm -rf "$BUILD_ROOT/src" "$PREFIX"
  mkdir -p "$BUILD_ROOT/src"
  # The bsdtar macOS ships reads 7z, so this needs nothing installed.
  tar -xf "$archive" -C "$BUILD_ROOT/src"
  src="$(echo "$BUILD_ROOT"/src/ImageMagick-*)"
  [[ -x "$src/configure" ]] || { echo "error: no configure script in $src." >&2; exit 1; }
  # As the formula does: lib/ImageMagick rather than lib/ImageMagick-7.1.2, the
  # layout Magick::discover and the rest of this script expect.
  sed -i '' 's/${PACKAGE_NAME}-${PACKAGE_BASE_VERSION}/${PACKAGE_NAME}/g' "$src/configure"

  # This is not Homebrew's sandbox, where only declared dependencies are
  # visible and their paths are passed in: the -I/-L flags stand in for the
  # latter (libltdl has no pkg-config file), and configure would link anything
  # installed on this Mac.  So every
  # optional delegate outside the formula's set is named and turned off, and the
  # result is checked below.
  (
    cd "$src"
    PKG_CONFIG="$BREW/bin/pkg-config" \
    PKG_CONFIG_PATH="$BREW/lib/pkgconfig" \
    CPPFLAGS="-I$BREW/include" \
    LDFLAGS="-L$BREW/lib" \
    ./configure \
      --prefix="$PREFIX" \
      --disable-dependency-tracking \
      --disable-silent-rules \
      --disable-opencl \
      --disable-openmp \
      --disable-docs \
      --enable-shared \
      --disable-static \
      --enable-hdri \
      --with-quantum-depth=16 \
      --with-modules \
      --with-openexr=yes \
      --with-webp=yes \
      --with-heic=yes \
      --without-x \
      --without-perl \
      --without-magick-plus-plus \
      --without-gslib \
      --with-gvc=no \
      --with-raw=no \
      --without-djvu \
      --without-fftw \
      --without-pango \
      --without-wmf \
      --without-jxl \
      --without-lqr \
      --without-openjp2 \
      --without-rsvg \
      --without-fontconfig \
      --without-raqm \
      --without-zip \
      --without-jbig \
      --without-dps \
      --without-fpx \
      --without-flif \
      --without-uhdr \
      --without-dmr \
      > "$BUILD_ROOT/configure.log" 2>&1 \
      || { echo "error: configure failed; see $BUILD_ROOT/configure.log" >&2; exit 1; }
    make -j"$(sysctl -n hw.ncpu)" install > "$BUILD_ROOT/make.log" 2>&1 \
      || { echo "error: make failed; see $BUILD_ROOT/make.log" >&2; exit 1; }
  )
  rm -rf "$BUILD_ROOT/src"
fi

if ! lipo -archs "$SRC_MAGICK" | grep -qw arm64; then
  echo "error: $SRC_MAGICK is not arm64 (found: $(lipo -archs "$SRC_MAGICK"))." >&2
  exit 1
fi

# The delegate set this build is meant to have - Homebrew's imagemagick plus
# openexr. More means configure found something on this Mac it should not ship;
# less means a library the formats rely on is missing.
EXPECTED_DELEGATES="bzlib freetype heic jng jpeg lcms ltdl lzma openexr png tiff webp xml zlib zstd"
delegates="$("$SRC_MAGICK" -version | sed -n 's/^Delegates (built-in): *//p' | tr -s ' ' | sed 's/ *$//')"
if [[ "$delegates" != "$EXPECTED_DELEGATES" ]]; then
  echo "error: the ImageMagick build has delegates" >&2
  echo "         $delegates" >&2
  echo "       expected" >&2
  echo "         $EXPECTED_DELEGATES" >&2
  exit 1
fi

echo "==> Vendoring ImageMagick $IM_VERSION from $PREFIX"

rm -rf "$DEST"
mkdir -p "$DEST/bin" "$DEST/lib"
cp "$SRC_MAGICK" "$DEST/bin/magick"
chmod u+w "$DEST/bin/magick"

# Homebrew builds ImageMagick with loadable coder/filter modules, so the format
# support lives in lib/ImageMagick/**/*.so, not in the binary.  Without these,
# magick can decode nothing at all.
cp -R "$PREFIX/lib/ImageMagick" "$DEST/lib/ImageMagick"
chmod -R u+w "$DEST/lib/ImageMagick"
# The .la files stay - ltdl resolves a coder by its .la, not the .so directly -
# but their libdir/dependency_libs record Homebrew Cellar paths.  Left alone,
# ltdl loads the Cellar .so instead of ours, which then links against the Cellar
# libMagickCore: two MagickCore instances in one process, and every coder
# registers into the copy the binary is not using ("no decode delegate" for
# every format).  Blanking libdir makes ltdl resolve dlname next to the .la.
sed -i '' -e "s|^libdir=.*|libdir=''|" -e "s|^dependency_libs=.*|dependency_libs=''|" \
  "$DEST/lib/ImageMagick"/modules-*/*/*.la
if [[ "$WITH_HEIC" -eq 0 ]]; then
  rm -f "$DEST/lib/ImageMagick"/modules-*/coders/heic.*
  echo "==> Dropped the HEIC/AVIF coder (--no-heic)"
fi

MODULES=()
while IFS= read -r m; do MODULES+=("$m"); done < <(find "$DEST/lib/ImageMagick" -name '*.so')
echo "==> Bundled ${#MODULES[@]} coder/filter modules"

# ---------------------------------------------------------------------------
# Walk the dependency graph and copy every non-system dylib into lib/
# ---------------------------------------------------------------------------

# Prints the install-name dependencies of a Mach-O, one per line, resolved to
# absolute paths.  Resolution happens against the ORIGINAL location of the
# binary ($2), never its copy in the bundle: a copied dylib sits next to
# dependencies that have not been copied yet, so resolving @rpath/@loader_path
# against the bundle would miss them.
deps_of() {
  local bin="$1" src="$2" src_dir rpaths
  src_dir="$(cd "$(dirname "$src")" && pwd)"
  # LC_RPATH entries of the original, used to expand @rpath/... references.
  rpaths="$(otool -l "$src" | awk '/LC_RPATH/{f=1} f&&/path /{print $2; f=0}')"

  otool -L "$bin" | tail -n +2 | awk '{print $1}' | while read -r dep; do
    case "$dep" in
      /usr/lib/*|/System/*) continue ;;                      # OS-provided, never bundle
      @loader_path/*|@executable_path/*) echo "$src_dir/${dep#*/}" ;;
      @rpath/*)
        local found=""
        while read -r rp; do
          [[ -n "$rp" ]] || continue
          rp="${rp//@loader_path/$src_dir}"
          rp="${rp//@executable_path/$src_dir}"
          if [[ -f "$rp/${dep#@rpath/}" ]]; then found="$rp/${dep#@rpath/}"; break; fi
        done <<< "$rpaths"
        # Homebrew dylibs often carry no LC_RPATH at all and rely on the
        # install prefix layout, so fall back to the prefix's lib directory.
        [[ -z "$found" && -f "$PREFIX/lib/${dep#@rpath/}" ]] && found="$PREFIX/lib/${dep#@rpath/}"
        [[ -z "$found" && -f "$(brew --prefix)/lib/${dep#@rpath/}" ]] && found="$(brew --prefix)/lib/${dep#@rpath/}"
        echo "$found"
        ;;
      *) echo "$dep" ;;
    esac
  done
}

# Breadth-first over the dependency graph.  Each queue entry is
# "<path in bundle>|<path it was copied from>"; a dylib already in lib/ has been
# seen and is not requeued.
# ${a[@]+"${a[@]}"} rather than "${a[@]}" throughout: under `set -u`, bash 3.2 -
# the bash macOS ships - treats an empty array expansion as an unbound variable,
# and an ImageMagick built without loadable modules has none.
queue=("$DEST/bin/magick|$SRC_MAGICK")
for m in ${MODULES[@]+"${MODULES[@]}"}; do
  queue+=("$m|$PREFIX/lib/ImageMagick/${m#"$DEST/lib/ImageMagick/"}")
done

while [[ ${#queue[@]} -gt 0 ]]; do
  entry="${queue[0]}"
  queue=("${queue[@]:1}")
  current="${entry%%|*}"
  current_src="${entry#*|}"
  while read -r dep; do
    [[ -n "$dep" ]] || continue
    if [[ ! -f "$dep" ]]; then
      echo "error: unresolved dependency '$dep' of $(basename "$current")" >&2
      exit 1
    fi
    base="$(basename "$dep")"
    if [[ ! -f "$DEST/lib/$base" ]]; then
      cp "$dep" "$DEST/lib/$base"
      chmod u+w "$DEST/lib/$base"
      queue+=("$DEST/lib/$base|$dep")
    fi
  done < <(deps_of "$current" "$current_src")
done

echo "==> Bundled $(find "$DEST/lib" -name '*.dylib' | wc -l | tr -d ' ') dylibs"

# ---------------------------------------------------------------------------
# Rewrite install names so nothing points back into /opt/homebrew
# ---------------------------------------------------------------------------

retarget() {
  local bin="$1"
  while read -r dep; do
    [[ -n "$dep" ]] || continue
    install_name_tool -change "$dep" "@rpath/$(basename "$dep")" "$bin" 2>/dev/null || true
  done < <(otool -L "$bin" | tail -n +2 | awk '{print $1}' | grep -vE '^(/usr/lib|/System|@rpath)/')
}

for lib in "$DEST"/lib/*.dylib; do
  install_name_tool -id "@rpath/$(basename "$lib")" "$lib" 2>/dev/null
  retarget "$lib"
  install_name_tool -add_rpath "@loader_path" "$lib" 2>/dev/null || true
done

for so in ${MODULES[@]+"${MODULES[@]}"}; do
  retarget "$so"
  # lib/ImageMagick/modules-Q16HDRI/<kind>/ -> lib/
  install_name_tool -add_rpath "@loader_path/../../.." "$so" 2>/dev/null || true
done

retarget "$DEST/bin/magick"
install_name_tool -add_rpath "@executable_path/../lib" "$DEST/bin/magick" 2>/dev/null

# Anything still referencing the Homebrew prefix or the build's own prefix would
# break on a user's machine.
if otool -L "$DEST/bin/magick" "$DEST"/lib/*.dylib ${MODULES[@]+"${MODULES[@]}"} | grep -qF -e "$BREW" -e "$PREFIX"; then
  echo "error: some install names still point at $BREW or $PREFIX:" >&2
  otool -L "$DEST/bin/magick" "$DEST"/lib/*.dylib ${MODULES[@]+"${MODULES[@]}"} | grep -F -e "$BREW" -e "$PREFIX" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Configuration files (delegates, type maps, policy, locale)
# ---------------------------------------------------------------------------

IM_CONFIG_DIR="$(basename "$(echo "$PREFIX"/etc/ImageMagick-*)")"
mkdir -p "$DEST/etc/$IM_CONFIG_DIR" "$DEST/share/$IM_CONFIG_DIR"
cp "$PREFIX/etc/$IM_CONFIG_DIR"/*.xml "$DEST/etc/$IM_CONFIG_DIR/"
cp -R "$PREFIX/share/$IM_CONFIG_DIR"/. "$DEST/share/$IM_CONFIG_DIR/"
MODULE_DIR="$(cd "$DEST/lib/ImageMagick" && echo modules-*)"
CONFIG_MODULE_DIR="$(cd "$DEST/lib/ImageMagick" && echo config-*)"

# Fonts and external delegates (ghostscript, ffmpeg, ...) are not bundled, so
# drop the type maps that point at Homebrew paths we are not shipping.
rm -f "$DEST/etc/$IM_CONFIG_DIR/type-ghostscript.xml" \
      "$DEST/etc/$IM_CONFIG_DIR/type-urw-base35.xml" \
      "$DEST/etc/$IM_CONFIG_DIR/type-urw-base35-type1.xml" \
      "$DEST/etc/$IM_CONFIG_DIR/type-dejavu.xml"

# No module may still point ltdl at the build machine's Homebrew tree.
# (configure.xml keeps its build-time paths - they are reported by
# `magick -list configure` and never used to resolve anything.)
if grep -lF "$(brew --prefix)" "$DEST/lib/ImageMagick"/modules-*/*/*.la >/dev/null 2>&1; then
  echo "error: module metadata still references $(brew --prefix):" >&2
  grep -lF "$(brew --prefix)" "$DEST/lib/ImageMagick"/modules-*/*/*.la >&2
  exit 1
fi

printf '%s\n' "$IM_VERSION" > "$DEST/VERSION"

# ---------------------------------------------------------------------------
# Sign (install_name_tool invalidated every signature we just touched)
# ---------------------------------------------------------------------------

echo "==> Signing with identity: $IDENTITY"
# --options runtime (hardened runtime) enables library validation, which only
# accepts dylibs signed by the same team - correct for a Developer ID build, but
# fatal for an ad-hoc one, where every signature has a different (empty) team.
sign_args=(--force --sign "$IDENTITY")
[[ "$IDENTITY" != "-" ]] && sign_args+=(--options runtime --timestamp)

# Apple's timestamp authority throttles bursts, and this loop is a burst of
# ~150 requests, so a failure here is usually transient - retry before giving up.
sign_one() {
  local f="$1" attempt out
  for attempt in 1 2 3; do
    # Capture rather than pipe: a pipeline would report grep's status, not codesign's.
    if out="$(codesign "${sign_args[@]}" "$f" 2>&1)"; then
      return 0
    fi
    [[ -t 1 ]] && echo >&2
    echo "   $(basename "$f"): ${out##*: }" >&2
    [[ $attempt -lt 3 ]] || break
    sleep $((attempt * 5))
  done
  echo "error: could not sign $f after 3 attempts: $out" >&2
  return 1
}

# Each signature is a round trip to timestamp.apple.com, so this loop runs for
# minutes. Count it out rather than going silent: on a terminal the line is
# rewritten in place, in a log (CI) it prints every 25 files instead of 150 times.
SIGN_TARGETS=("$DEST"/lib/*.dylib ${MODULES[@]+"${MODULES[@]}"} "$DEST/bin/magick")
SIGN_TOTAL=${#SIGN_TARGETS[@]}
signed=0
for f in "${SIGN_TARGETS[@]}"; do
  signed=$((signed + 1))
  if [[ -t 1 ]]; then
    printf '\r    [%d/%d] %-40.40s' "$signed" "$SIGN_TOTAL" "$(basename "$f")"
  elif (( signed % 25 == 0 || signed == SIGN_TOTAL )); then
    echo "    [$signed/$SIGN_TOTAL]"
  fi
  sign_one "$f"
done
if [[ -t 1 ]]; then printf '\r    [%d/%d] done%-40s\n' "$SIGN_TOTAL" "$SIGN_TOTAL" ""; fi
codesign --verify --strict "$DEST/bin/magick"

# ---------------------------------------------------------------------------
# Smoke test: the bundle must run with no Homebrew and no DYLD help
# ---------------------------------------------------------------------------

echo "==> Smoke testing the bundle"
# env -i: no Homebrew on PATH, no DYLD_* crutches - exactly a user's machine.
smoke_env=(
  MAGICK_HOME="$DEST"
  MAGICK_CONFIGURE_PATH="$DEST/lib/ImageMagick/$CONFIG_MODULE_DIR:$DEST/etc/$IM_CONFIG_DIR:$DEST/share/$IM_CONFIG_DIR"
  MAGICK_CODER_MODULE_PATH="$DEST/lib/ImageMagick/$MODULE_DIR/coders"
  MAGICK_FILTER_MODULE_PATH="$DEST/lib/ImageMagick/$MODULE_DIR/filters"
)
smoke_out="$(env -i PATH=/usr/bin:/bin "${smoke_env[@]}" "$DEST/bin/magick" -size 16x16 xc:red png:- | wc -c)"
[[ "$smoke_out" -gt 100 ]] || { echo "error: bundled magick produced no PNG output." >&2; exit 1; }
for fmt in JPEG PNG TIFF WEBP; do
  env -i PATH=/usr/bin:/bin "${smoke_env[@]}" "$DEST/bin/magick" -list format 2>/dev/null | grep -qE "^ *$fmt\*? " \
    || echo "warning: bundled magick reports no $fmt support" >&2
done
# EXR is the reason this script builds ImageMagick itself, so it is checked by a
# real round trip rather than listed: a float gradient written and read back.
exr_dir="$(mktemp -d)"
env -i PATH=/usr/bin:/bin "${smoke_env[@]}" "$DEST/bin/magick" \
  -size 16x16 gradient:red-blue -depth 32 -define quantum:format=floating-point "$exr_dir/smoke.exr" \
  && env -i PATH=/usr/bin:/bin "${smoke_env[@]}" "$DEST/bin/magick" identify "$exr_dir/smoke.exr" | grep -q ' EXR 16x16 ' \
  || { echo "error: the bundled magick cannot write and read EXR." >&2; rm -rf "$exr_dir"; exit 1; }
rm -rf "$exr_dir"
echo "==> EXR round trip works"

# The bundle is packaged into an .app and installed wherever the user drops it,
# so prove it still runs from a path it has never seen.
RELOCATED="$(mktemp -d)"
trap 'rm -rf "$RELOCATED"' EXIT
cp -R "$DEST" "$RELOCATED/magick"
env -i PATH=/usr/bin:/bin \
  MAGICK_HOME="$RELOCATED/magick" \
  MAGICK_CONFIGURE_PATH="$RELOCATED/magick/lib/ImageMagick/$CONFIG_MODULE_DIR:$RELOCATED/magick/etc/$IM_CONFIG_DIR:$RELOCATED/magick/share/$IM_CONFIG_DIR" \
  MAGICK_CODER_MODULE_PATH="$RELOCATED/magick/lib/ImageMagick/$MODULE_DIR/coders" \
  MAGICK_FILTER_MODULE_PATH="$RELOCATED/magick/lib/ImageMagick/$MODULE_DIR/filters" \
  "$RELOCATED/magick/bin/magick" -size 16x16 gradient:red-blue "$RELOCATED/out.jpg" \
  || { echo "error: the bundle does not run from a relocated path." >&2; exit 1; }
[[ -s "$RELOCATED/out.jpg" ]] || { echo "error: relocated bundle wrote no output." >&2; exit 1; }

echo "==> resources/mac/magick ready ($(du -sh "$DEST" | cut -f1), ImageMagick $IM_VERSION)"
