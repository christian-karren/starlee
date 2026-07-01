#!/bin/sh
# Compile and run Swift XCTest suite for the Starlee GUI.
# Produces a standalone test executable (no Xcode project required).
# No running Starlee service required — URLSession is mocked at the boundary.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
GUI="$ROOT/gui"
TESTS="$GUI/Tests"
OUT="$ROOT/target/swift-tests"
BINARY="$OUT/StarleeGUITests"

mkdir -p "$OUT"

XCODE_DEV=$(xcode-select -p)
SDK=$(xcrun --show-sdk-path --sdk macosx)
XCTEST_FW_PATH="${XCODE_DEV}/Platforms/MacOSX.platform/Developer/Library/Frameworks"
XCTEST_LIB_PATH="${XCODE_DEV}/Platforms/MacOSX.platform/Developer/usr/lib"

# All production Swift files (excluding the @main entry point), followed by tests.
# Feed paths through xargs -0 so checkouts in folders with spaces still compile.
SOURCE_LIST="$OUT/swift-sources.list"
{
  find "$GUI" -maxdepth 1 -name "*.swift" ! -name "StarleeMain.swift" -print0
  find "$TESTS" -name "*.swift" -print0
} | sort -z > "$SOURCE_LIST"

echo "Compiling Swift tests..."
xargs -0 xcrun swiftc \
  -sdk "$SDK" \
  -F "$XCTEST_FW_PATH" \
  -I "$XCTEST_LIB_PATH" \
  -L "$XCTEST_LIB_PATH" \
  -framework AppKit \
  -framework Foundation \
  -framework UserNotifications \
  -framework XCTest \
  -lXCTestSwiftSupport \
  -Xlinker -rpath -Xlinker "$XCTEST_FW_PATH" \
  -Xlinker -rpath -Xlinker "$XCTEST_LIB_PATH" \
  -o "$BINARY" \
  < "$SOURCE_LIST"

echo "Running Swift tests..."
export DYLD_FRAMEWORK_PATH="${XCTEST_FW_PATH}${DYLD_FRAMEWORK_PATH:+:$DYLD_FRAMEWORK_PATH}"
export DYLD_LIBRARY_PATH="${XCTEST_LIB_PATH}${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
exec "$BINARY"
