#!/usr/bin/env bash
# shellcheck disable=SC2016 # Required workflow snippets are literal strings.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
recovery="$root/.github/workflows/recover-v0.1.0.yml"
[[ -f "$ci" && -f "$release" && -f "$recovery" ]] || {
  echo "error: CI, release, and v0.1.0 recovery workflows are required" >&2
  exit 1
}

python3 - "$ci" "$release" "$recovery" <<'PY'
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
  'org.opencontainers.image.licenses=(MIT OR Apache-2.0) AND BSD-3-Clause' \
  'org.dekopon.distribution.notices=embedded:dekopon.third-party-notices' \
  'embed-license-bundle.py" write' \
  'embed-license-bundle.py" verify-text' \
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
if grep -Eq 'CARGO_TARGET_DIR|cargo clean' "$ci" "$release" "$recovery"; then
  echo "error: workflow overrides Cargo target policy or cleans shared build state" >&2
  exit 1
fi

for required in \
  'workflow_dispatch:' \
  'actions: read' \
  'attestations: read' \
  'contents: write' \
  'id-token: write' \
  'packages: write' \
  'SOURCE_SHA: "8bf0ba18f9240d5924d008d09283a5cd7c879f84"' \
  'SOURCE_RUN_ID: "32733258088"' \
  'SOURCE_BUILD_JOB_ID: "97450107488"' \
  'SOURCE_ATTEST_JOB_ID: "97456266494"' \
  'SOURCE_ARTIFACT_ID: "9522912673"' \
  'SOURCE_ARTIFACT_ARCHIVE_DIGEST: "sha256:048a291a23288c083790aa1881119a70d4d4efb53b95b7d181c979a86cae2d23"' \
  'run-id: "32733258088"' \
  '--source-digest "$SOURCE_SHA"' \
  '--source-ref "refs/tags/$TAG"' \
  'gh release create "$TAG" --verify-tag --draft' \
  'org.opencontainers.image.revision=$SOURCE_SHA' \
  'dist/ripgrep-provider.wasm:application/wasm' \
  'provider-ripgrep/versions' \
  'This PATCH is the recovery transaction' \
  'post-finalization anonymous read-only verification' \
  'needs.finalize.result != '\''success'\''' \
  'owned manifest is missing, shared, or has another tag/version'; do
  grep -Fq -- "$required" "$recovery" || {
    echo "error: recovery workflow omits required interlock: $required" >&2
    exit 1
  }
done

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
if text.count('(MIT OR Apache-2.0) AND BSD-3-Clause') < 3:
    raise SystemExit('error: BSD-inclusive binary license expression is not verified end-to-end')
if text.count('embedded:dekopon.third-party-notices') < 2:
    raise SystemExit('error: embedded-notice annotation is not verified end-to-end')
PY

python3 - "$recovery" <<'PY'
import pathlib
import re
import sys
text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
preflight = text.index("  preflight:")
draft = text.index("  draft:")
publish = text.index("  publish:")
finalize = text.index("  finalize:")
final_patch = text.index("# This PATCH is the recovery transaction's final mutation.")
post = text.index("  verify_final:")
cleanup = text.index("  cleanup_failed_recovery:")
if not preflight < draft < publish < finalize < final_patch < post < cleanup:
    raise SystemExit("error: recovery preflight/mutation/finalization/cleanup ordering drifted")
if text.count("permissions:") != 1:
    raise SystemExit("error: recovery permissions must be explicit only once")
permissions = """permissions:
  actions: read
  attestations: read
  contents: write
  id-token: write
  packages: write
"""
if permissions not in text:
    raise SystemExit("error: recovery permissions drifted")
if text.count('gh release create "$TAG"') != 1:
    raise SystemExit("error: recovery draft creation cardinality drifted")
if text.count('"$RUNNER_TEMP/oras-bin" push "$ref"') != 1:
    raise SystemExit("error: recovery OCI push cardinality drifted")
if text.count("dist/ripgrep-provider.wasm:application/wasm") != 1:
    raise SystemExit("error: recovery OCI layer cardinality drifted")
if "actions/upload-artifact" in text:
    raise SystemExit("error: recovery must not substitute or re-upload source-run bytes")
if re.search(r"cargo (?:build|test|check)|prepare-release-assets|reproducible-build", text):
    raise SystemExit("error: recovery must not rebuild provider bytes")
if "staging" in text.lower() or re.search(r":(?:tmp|temp)(?:[\"'@\s]|$)", text.lower()):
    raise SystemExit("error: recovery names a forbidden secondary tag")
post_text = text[post:cleanup]
for mutation in ("--method PATCH", "--method DELETE", "gh release create", ' push "$ref"'):
    if mutation in post_text:
        raise SystemExit(f"error: post-finalization verification can mutate via {mutation}")
if text.count("run-id: \"32733258088\"") < 2:
    raise SystemExit("error: recovery must repeatedly download by fixed source run ID")
if text.count("sha256:$COMPONENT_SHA256") < 4:
    raise SystemExit("error: component digest is not verified and documented end-to-end")
PY

temporary=$(mktemp -d "${TMPDIR:-/tmp}/ripgrep-oci-verifier.XXXXXX")
trap 'rm -rf "$temporary"' EXIT
printf 'component fixture\n' >"$temporary/ripgrep-provider.wasm"
python3 - "$temporary/manifest.json" "$temporary/ripgrep-provider.wasm" <<'PY'
import hashlib
import json
import pathlib
import sys
component = pathlib.Path(sys.argv[2]).read_bytes()
manifest = {
    "schemaVersion": 2,
    "artifactType": "application/vnd.dekopon.provider.v1+wasm",
    "config": {"size": 2},
    "annotations": {
        "org.opencontainers.image.source": "https://github.com/dekopon-agents/dekopon-provider-ripgrep",
        "org.opencontainers.image.version": "0.1.0",
        "org.opencontainers.image.revision": "a" * 40,
        "org.opencontainers.image.licenses": "(MIT OR Apache-2.0) AND BSD-3-Clause",
        "org.dekopon.distribution.notices": "embedded:dekopon.third-party-notices",
        "org.dekopon.release.run": "1:1",
        "org.dekopon.release.url": "https://github.com/dekopon-agents/dekopon-provider-ripgrep/releases/tag/v0.1.0",
        "org.dekopon.provider.capability": "ripgrep.search",
    },
    "layers": [{
        "mediaType": "application/wasm",
        "digest": "sha256:" + hashlib.sha256(component).hexdigest(),
        "size": len(component),
        "annotations": {"org.opencontainers.image.title": "ripgrep-provider.wasm"},
    }],
}
pathlib.Path(sys.argv[1]).write_text(json.dumps(manifest), encoding="utf-8")
PY
"$root/scripts/verify-oci-manifest.py" "$temporary/manifest.json" \
  "$temporary/ripgrep-provider.wasm" 1:1 "$(printf 'a%.0s' {1..40})"
python3 - "$temporary/manifest.json" <<'PY'
import json
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
manifest = json.loads(path.read_text(encoding="utf-8"))
manifest["annotations"]["org.opencontainers.image.licenses"] = "MIT OR Apache-2.0"
path.write_text(json.dumps(manifest), encoding="utf-8")
PY
if "$root/scripts/verify-oci-manifest.py" "$temporary/manifest.json" \
  "$temporary/ripgrep-provider.wasm" 1:1 "$(printf 'a%.0s' {1..40})" >/dev/null 2>&1; then
  echo 'error: OCI verifier accepted the incomplete binary license expression' >&2
  exit 1
fi

printf 'workflow SHA pins, immutable release transaction, notices, OCI verifier, and cleanup layout passed\n'
