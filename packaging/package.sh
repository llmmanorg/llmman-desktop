#!/usr/bin/env bash
# Bundles target/<target>/release/llmman-desktop with llmman's own release
# binary for the same target into dist/:
#
#   packaging/package.sh <target> <version> <llmman-tag>
#
#   aarch64-apple-darwin      llmman-desktop-<target>.zip     llmman.app
#   *-unknown-linux-gnu       llmman-desktop-<target>.tar.gz  llmman-desktop/{llmman-desktop,llmman}
#   *-pc-windows-msvc         llmman-desktop-<target>.zip     llmman-desktop/{llmman-desktop,llmman}.exe
#
# The app runs the `llmman` next to it, so the pair is the unit. llmman's
# binary is checked against its release's checksums.txt.

set -euo pipefail
cd -- "$(dirname -- "$0")/.."

die() {
	printf 'package.sh: %s\n' "$*" >&2
	exit 1
}

[ $# -eq 3 ] || die "usage: package.sh <target> <version> <llmman-tag>"
target=$1 version=$2 tag=$3
case "$target" in
*-windows-*) ext=.exe ;;
*) ext= ;;
esac
asset="llmman-$target$ext"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for f in "$asset" checksums.txt; do
	curl -fsSL --retry 3 -o "$work/$f" "https://github.com/llmmanorg/llmman/releases/download/$tag/$f"
done
if command -v sha256sum >/dev/null; then sha=(sha256sum); else sha=(shasum -a 256); fi
(cd "$work" && grep "  $asset\$" checksums.txt | "${sha[@]}" -c -) || die "$asset: checksum mismatch"
chmod +x "$work/$asset"
"$work/$asset" --version

bin="target/$target/release/llmman-desktop$ext"
[ -f "$bin" ] || bin="target/release/llmman-desktop$ext"
mkdir -p dist
out="$PWD/dist/llmman-desktop-$target"

case "$target" in
*-apple-darwin)
	app="$work/llmman.app"
	mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
	cp "$bin" "$app/Contents/MacOS/llmman-desktop"
	cp "$work/$asset" "$app/Contents/MacOS/llmman"
	# The manatee mark, pinned as llmman's build.rs pins it.
	curl -fsSL -o "$work/mark.png" https://github.com/llmmanorg/llmman/releases/download/docs-assets/llmman-mark.png
	(cd "$work" && echo "c6036711a349dc8f8b2b4405be90b342b477dbcf4a7d95eb7d05b42702a241d5  mark.png" | shasum -a 256 -c -)
	mkdir "$work/llmman.iconset"
	for size in 16 32 128 256 512; do
		sips -z $size $size "$work/mark.png" --out "$work/llmman.iconset/icon_${size}x${size}.png" >/dev/null
		sips -z $((size * 2)) $((size * 2)) "$work/mark.png" --out "$work/llmman.iconset/icon_${size}x${size}@2x.png" >/dev/null
	done
	iconutil -c icns -o "$app/Contents/Resources/llmman.icns" "$work/llmman.iconset"
	cat >"$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key><string>llmman-desktop</string>
	<key>CFBundleIdentifier</key><string>org.llmman.desktop</string>
	<key>CFBundleName</key><string>llmman</string>
	<key>CFBundleIconFile</key><string>llmman</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleShortVersionString</key><string>$version</string>
	<key>CFBundleVersion</key><string>$version</string>
	<key>LSMinimumSystemVersion</key><string>11.0</string>
	<key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
	codesign --force --deep --sign - "$app"
	rm -f "$out.zip"
	ditto -c -k --norsrc --noextattr --keepParent "$app" "$out.zip"
	echo "$out.zip"
	;;
*-linux-*)
	mkdir "$work/llmman-desktop"
	cp "$bin" "$work/llmman-desktop/llmman-desktop"
	cp "$work/$asset" "$work/llmman-desktop/llmman"
	tar -C "$work" -czf "$out.tar.gz" llmman-desktop
	echo "$out.tar.gz"
	;;
*-windows-*)
	mkdir "$work/llmman-desktop"
	cp "$bin" "$work/llmman-desktop/llmman-desktop.exe"
	cp "$work/$asset" "$work/llmman-desktop/llmman.exe"
	rm -f "$out.zip"
	powershell -NoProfile -Command "Compress-Archive -Path '$(cygpath -w "$work/llmman-desktop")' -DestinationPath '$(cygpath -w "$out.zip")'"
	echo "$out.zip"
	;;
*) die "unsupported target $target" ;;
esac
