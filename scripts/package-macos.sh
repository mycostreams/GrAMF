#!/usr/bin/env bash
# Wraps target/release/GrAMF in GrAMF.app (ad-hoc signed, not notarized) and zips it.
# Usage: scripts/package-macos.sh GrAMF-macos-arm64.zip
set -euo pipefail
out="${1:?zip name}"
version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
app=target/release/GrAMF.app

rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/GrAMF "$app/Contents/MacOS/GrAMF"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>GrAMF</string>
  <key>CFBundleDisplayName</key><string>GrAMF</string>
  <key>CFBundleIdentifier</key><string>org.mycostreams.gramf</string>
  <key>CFBundleExecutable</key><string>GrAMF</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --deep --sign - "$app"
rm -f "$out"
ditto -c -k --keepParent "$app" "$out"
echo "packaged $out ($version)"
