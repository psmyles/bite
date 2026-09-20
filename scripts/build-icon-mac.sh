#!/usr/bin/env bash
#
# Compiles build/icons/bite.icon (Icon Composer) into the macOS icon artifacts.
#
#   scripts/build-icon-mac.sh
#
# There is no packaged macOS app yet, so nothing consumes these automatically; the
# script is kept ready for one, and its outputs are what such a bundle would ship.
#
# Produces, in build/icons/mac/:
#
#   Assets.car  - the compiled catalog. Belongs at Contents/Resources/Assets.car and
#                 is named by the bundle's CFBundleIconName; this is what macOS 26+
#                 reads to draw the layered, Liquid Glass icon.
#   icon.icns   - the flattened fallback, used by older macOS, the dmg, and
#                 anything that reads CFBundleIconFile.
#
# Needs full Xcode (actool, swiftc) - the Command Line Tools alone do not ship
# them.
#
# Why the icns is not simply actool's: actool does emit a bite.icns next to the
# catalog, but for an .icon source it stops at 256x256, which is a visible
# downgrade from the 1024x1024 icon this replaced. So the icns is built from the
# catalog instead: a stub .app carrying Assets.car is handed to NSWorkspace,
# which composes the icon exactly as Finder would - glass, shading, squircle and
# all - and each iconset size is rendered from that, so the small sizes get the
# catalog's own small-size renditions rather than a downscaled 1024.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SOURCE="build/icons/bite.icon"
OUT="build/icons/mac"
# The catalog's icon name, and therefore the CFBundleIconName the app has to
# declare. It is the .icon's basename, so the two move together.
ICON_NAME="$(basename "$SOURCE" .icon)"

step() { echo; echo "==> $*"; }
fail() { echo "error: $*" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || fail "macOS only."
[[ -d "$SOURCE" ]] || fail "$SOURCE not found."
command -v xcrun >/dev/null || fail "xcrun not found - install Xcode."
ACTOOL="$(xcrun --find actool 2>/dev/null)" || fail "actool not found. It ships with full Xcode, not the Command Line Tools: install Xcode and run 'sudo xcode-select -s /Applications/Xcode.app'."
# /usr/bin/swiftc, not `xcrun --find swiftc`: the shim matches the toolchain to
# the running OS, while xcrun hands back Xcode's own toolchain, which refuses to
# load its standard library when macOS is newer than the bundled SDK.
SWIFTC="$(command -v swiftc || true)"
[[ -n "$SWIFTC" ]] || SWIFTC="$(xcrun --find swiftc 2>/dev/null)" || fail "swiftc not found - install Xcode."

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ---------------------------------------------------------------------------
# Compile the catalog
# ---------------------------------------------------------------------------

step "Compiling $SOURCE"

rm -rf "$OUT"
mkdir -p "$OUT" "$TMP/car"
"$ACTOOL" "$SOURCE" \
  --compile "$TMP/car" \
  --app-icon "$ICON_NAME" \
  --output-partial-info-plist "$TMP/icon.plist" \
  --platform macosx \
  --minimum-deployment-target 26.0 \
  --output-format human-readable-text >/dev/null

[[ -f "$TMP/car/Assets.car" ]] || fail "actool produced no Assets.car."
cp "$TMP/car/Assets.car" "$OUT/Assets.car"

# ---------------------------------------------------------------------------
# Render the icns from the catalog
# ---------------------------------------------------------------------------

step "Rendering icon.icns"

# A bundle LaunchServices considers unlaunchable is drawn with the "prohibited"
# badge over it, so the stub needs a real executable - any Mach-O will do.
STUB="$TMP/Bite.app"
mkdir -p "$STUB/Contents/MacOS" "$STUB/Contents/Resources"
cp "$OUT/Assets.car" "$STUB/Contents/Resources/Assets.car"
cp /bin/echo "$STUB/Contents/MacOS/Stub"
cat > "$STUB/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>com.bite.iconstub</string>
  <key>CFBundleName</key><string>Bite</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>Stub</string>
  <key>CFBundleIconName</key><string>$ICON_NAME</string>
</dict></plist>
PLIST

cat > "$TMP/render.swift" <<'SWIFT'
// Draws a bundle's composed icon at each size an .iconset needs.
//   render <app bundle> <output directory>
import AppKit

let app = CommandLine.arguments[1]
let outDir = URL(fileURLWithPath: CommandLine.arguments[2])

func bitmap(_ size: Int) -> NSBitmapImageRep? {
  return NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
}

// The icon server fills the larger representations in asynchronously, and a
// representation it has not produced yet draws as a flat grey dashed outline.
// That placeholder has no colour in it, so any appreciable saturation means the
// real rendition arrived.
func isPlaceholder(_ rep: NSBitmapImageRep) -> Bool {
  guard let data = rep.bitmapData else { return true }
  let pixels = rep.pixelsWide * rep.pixelsHigh
  let stride = rep.samplesPerPixel
  var coloured = 0
  for i in 0..<pixels {
    let p = data + i * stride
    if stride > 3 && p[3] < 128 { continue }
    let hi = max(p[0], max(p[1], p[2])), lo = min(p[0], min(p[1], p[2]))
    if Int(hi) - Int(lo) > 32 { coloured += 1 }
  }
  return coloured * 100 < pixels
}

func render(_ size: Int) -> NSBitmapImageRep? {
  // Re-fetching the icon each attempt is what picks up the representations the
  // icon server has finished since the last one.
  let icon = NSWorkspace.shared.icon(forFile: app)
  guard let rep = bitmap(size) else { return nil }
  rep.size = NSSize(width: size, height: size)
  NSGraphicsContext.saveGraphicsState()
  NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
  // NSImage picks the representation matching the destination rect, so each
  // size comes from the catalogue's own rendition rather than a resampled one.
  icon.draw(
    in: NSRect(x: 0, y: 0, width: size, height: size), from: .zero,
    operation: .sourceOver, fraction: 1.0)
  NSGraphicsContext.restoreGraphicsState()
  return rep
}

func fail(_ message: String) -> Never {
  FileHandle.standardError.write("\(message)\n".data(using: .utf8)!)
  exit(1)
}

for size in [16, 32, 64, 128, 256, 512, 1024] {
  var attempt = 0
  var result: NSBitmapImageRep?
  while attempt < 100 {
    guard let rep = render(size) else { fail("cannot allocate \(size)px bitmap") }
    if !isPlaceholder(rep) {
      result = rep
      break
    }
    attempt += 1
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
  }
  guard let rep = result else {
    fail("the icon server never produced the \(size)px representation")
  }
  guard let png = rep.representation(using: .png, properties: [:]) else {
    fail("cannot encode \(size)px png")
  }
  try png.write(to: outDir.appendingPathComponent("\(size).png"))
}
SWIFT

"$SWIFTC" -O "$TMP/render.swift" -o "$TMP/render"
mkdir -p "$TMP/png"
"$TMP/render" "$STUB" "$TMP/png"

ICONSET="$TMP/icon.iconset"
mkdir -p "$ICONSET"
# iconset name -> rendered pixel size
for entry in \
  "icon_16x16:16" "icon_16x16@2x:32" \
  "icon_32x32:32" "icon_32x32@2x:64" \
  "icon_128x128:128" "icon_128x128@2x:256" \
  "icon_256x256:256" "icon_256x256@2x:512" \
  "icon_512x512:512" "icon_512x512@2x:1024"; do
  cp "$TMP/png/${entry##*:}.png" "$ICONSET/${entry%%:*}.png"
done

iconutil --convert icns --output "$OUT/icon.icns" "$ICONSET"

# An icns missing its large sizes still converts cleanly, so walk the chunk
# table and insist on the 1024x1024 entry (ic10) rather than trusting the exit
# code.
node -e '
  const fs = require("fs");
  const d = fs.readFileSync(process.argv[1]);
  const types = [];
  for (let off = 8; off + 8 <= d.length; ) {
    types.push(d.toString("latin1", off, off + 4));
    const len = d.readUInt32BE(off + 4);
    if (len < 8) break;
    off += len;
  }
  if (!types.includes("ic10")) {
    console.error(`icon.icns has no 1024x1024 entry (found: ${types.join(", ")})`);
    process.exit(1);
  }
' "$OUT/icon.icns" || fail "icon.icns is missing its largest representation."

echo
echo "==> $OUT/icon.icns ($(du -h "$OUT/icon.icns" | cut -f1)), $OUT/Assets.car ($(du -h "$OUT/Assets.car" | cut -f1))"
