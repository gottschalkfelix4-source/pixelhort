#!/usr/bin/env sh
# Type-checks and smoke-tests the Foundation-only Core layer with the official Swift Linux image.
# Usage (from mobile/ios):  sh Tools/check-core.sh
set -e
cd "$(dirname "$0")/.."
docker run --rm -v "$PWD:/ios:ro" swift:6.0 bash -c '
  set -e
  CORE=$(find /ios/Pixelhort/Core -name "*.swift")
  swiftc -swift-version 5 -typecheck $CORE
  swiftc -swift-version 6 -typecheck $CORE
  swiftc -swift-version 5 -o /tmp/core-smoke $CORE /ios/Tools/CoreSmokeTest/main.swift
  /tmp/core-smoke
'
