#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
./scripts/validate-source.sh
./scripts/build-component.sh
./scripts/inspect-component.sh
./scripts/test-raw-component.sh
./scripts/test-broker-testkit.sh
./scripts/prepare-release-assets.sh "" "$root/dist"
./scripts/verify-release-assets.sh "$root/dist"
printf 'all source, component, raw-host, broker-host, and resource gates passed\n'
