#!/usr/bin/env bash
# shellcheck disable=SC2016 # Required workflow snippets are literal strings.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
[[ -f "$ci" && -f "$release" ]] || { echo "error: CI and release workflows are required" >&2; exit 1; }

python3 - "$ci" "$release" <<'PY'
import pathlib
import re
import sys
for path_string in sys.argv[1:]:
    path = pathlib.Path(path_string)
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        match = re.match(r"\s*uses:\s*([^\s#]+)", line)
        if match and not re.fullmatch(r"[^@]+@[0-9a-f]{40}", match.group(1)):
            raise SystemExit(f"error: {path}:{number}: Action is not full-SHA pinned")
PY

for required in \
  'tags:' \
  '"v0.1.0"' \
  'test "$(git cat-file -t "refs/tags/$GITHUB_REF_NAME")" = tag' \
  'git merge-base --is-ancestor "$GITHUB_SHA" refs/remotes/origin/main' \
  'application/vnd.dekopon.provider.v1+wasm' \
  'dist/ripgrep-provider.wasm:application/wasm' \
  'org.dekopon.release.run' \
  'manifest_digest' \
  'anonymous-pull' \
  'provider-ripgrep/versions' \
  '([.assets[].name] | sort)' \
  'ripgrep-provider.wasm.sha256' \
  'subject-path: dist/*' \
  'needs.finalize.result != '\''success'\''' \
  'manifest is missing, shared, or has another tag/version' \
  'This PATCH is the release transaction' \
  'draft: false'; do
  grep -Fq "$required" "$release" || {
    echo "error: release workflow omits required interlock: $required" >&2
    exit 1
  }
done

if grep -Eq 'ghcr[.]io/dekopon-agents/provider-ripgrep:(latest|staging|tmp|temp)' "$release"; then
  echo "error: release workflow names a mutable or secondary provider package/tag" >&2
  exit 1
fi
if grep -Eq 'CARGO_TARGET_DIR|cargo clean' "$ci" "$release"; then
  echo "error: workflow overrides Cargo target policy or cleans shared build state" >&2
  exit 1
fi

python3 - "$release" <<'PY'
import pathlib
import sys
text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
anonymous = text.index('Recheck draft assets and every anonymous OCI byte, then finalize')
final_patch = text.index('# This PATCH is the release transaction')
cleanup = text.index('cleanup_failed_release:')
if not anonymous < final_patch < cleanup:
    raise SystemExit('error: public verification/finalization/cleanup ordering drifted')
if text.count('gh release create v0.1.0') != 1:
    raise SystemExit('error: release draft creation cardinality drifted')
if text.count('dist/ripgrep-provider.wasm:application/wasm') != 1:
    raise SystemExit('error: OCI provider push cardinality drifted')
PY

printf 'workflow SHA pins, immutable release transaction, and cleanup layout passed\n'
