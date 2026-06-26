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

# All production Swift files (excluding the @main entry point)
SOURCES=$(find "$GUI" -maxdepth 1 -name "*.swift" ! -name "StarleeMain.swift" | sort | tr '\n' ' ')
# All test Swift files (main.swift provides the entry point)
TEST_SOURCES=$(find "$TESTS" -name "*.swift" | sort | tr '\n' ' ')

echo "Compiling Swift tests..."
# main.swift in Tests/ acts as the entry point; no -parse-as-library.
# shellcheck disable=SC2086
xcrun swiftc \
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
  $SOURCES $TEST_SOURCES \
  -o "$BINARY"

echo "Running Swift tests..."
export DYLD_FRAMEWORK_PATH="${XCTEST_FW_PATH}${DYLD_FRAMEWORK_PATH:+:$DYLD_FRAMEWORK_PATH}"
export DYLD_LIBRARY_PATH="${XCTEST_LIB_PATH}${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
exec "$BINARY"
