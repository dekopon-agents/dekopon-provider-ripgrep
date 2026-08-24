#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
./scripts/validate-source.sh
./scripts/build-component.sh
./scripts/inspect-component.sh
./scripts/test-raw-component.sh
./scripts/test-direct-host.sh
./scripts/test-resource-limits.sh
./scripts/test-broker-testkit.sh
./scripts/prepare-release-assets.sh 0.1.0 "$root/dist"
./scripts/verify-release-assets.sh "$root/dist"
printf 'all v0.1.0 source, component, direct-host, testkit, and resource gates passed\n'
