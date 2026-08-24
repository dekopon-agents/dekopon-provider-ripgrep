#!/usr/bin/env bash
# shellcheck disable=SC2016 # Required workflow snippets are literal strings.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
ci="$root/.github/workflows/ci.yml"
release="$root/.github/workflows/release.yml"
finalizer="$root/.github/workflows/recover-v0.1.0.yml"
verifier="$root/scripts/verify-oci-manifest.py"
[[ -f "$ci" && -f "$release" && -f "$finalizer" && -f "$verifier" ]] || {
  echo "error: CI, release, residual finalizer, and OCI verifier are required" >&2
  exit 1
}

python3 - "$ci" "$release" "$finalizer" <<'PY'
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
  'ripgrep-provider.wasm:application/wasm' \
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
  echo "error: release workflow names a mutable or secondary provider package tag" >&2
  exit 1
fi
if grep -Eq 'CARGO_TARGET_DIR|cargo clean' "$ci" "$release" "$finalizer"; then
  echo "error: workflow overrides Cargo target policy or uses cargo clean" >&2
  exit 1
fi

python3 - "$release" <<'PY'
import pathlib
import sys
text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
ghcr_start = text.index("  ghcr:")
finalize_start = text.index("  finalize:", ghcr_start)
ghcr = text[ghcr_start:finalize_start]
anonymous = text.index("Recheck draft assets and every anonymous OCI byte, then finalize")
final_patch = text.index("# This PATCH is the release transaction")
cleanup = text.index("cleanup_failed_release:")
if not ghcr_start < finalize_start <= anonymous < final_patch < cleanup:
    raise SystemExit("error: release publication/finalization/cleanup ordering drifted")
if "# Draft releases are visible to this job only with writable contents access." not in ghcr:
    raise SystemExit("error: GHCR draft-read permission rationale is missing")
if ghcr.count("contents: write") != 1 or ghcr.count("packages: write") != 1:
    raise SystemExit("error: GHCR draft/package permissions drifted")
cd_dist = ghcr.index("cd dist")
push = ghcr.index('"$RUNNER_TEMP/oras-bin" push "$ref"')
layer = ghcr.index("ripgrep-provider.wasm:application/wasm", push)
if not cd_dist < push < layer:
    raise SystemExit("error: ORAS must push the bare component path from within dist")
if "dist/ripgrep-provider.wasm:application/wasm" in ghcr:
    raise SystemExit("error: ORAS push would preserve a workspace path in the layer title")
if text.count("gh release create v0.1.0") != 1:
    raise SystemExit("error: release draft creation cardinality drifted")
if text.count('"$RUNNER_TEMP/oras-bin" push "$ref"') != 1:
    raise SystemExit("error: release OCI push cardinality drifted")
if text.count("ripgrep-provider.wasm:application/wasm") != 1:
    raise SystemExit("error: release OCI layer cardinality drifted")
if text.count("(MIT OR Apache-2.0) AND BSD-3-Clause") < 3:
    raise SystemExit("error: BSD-inclusive binary license is not verified end-to-end")
if text.count("embedded:dekopon.third-party-notices") < 2:
    raise SystemExit("error: embedded-notice annotation is not verified end-to-end")
PY

for required in \
  'workflow_dispatch:' \
  'CONTROL_BASE_SHA: "8556da5846f25ad6f2a3a5cbe2b190443c9dc86b"' \
  'SOURCE_SHA: "8bf0ba18f9240d5924d008d09283a5cd7c879f84"' \
  'TAG_OBJECT_SHA: "55e3a18f84d26ac2bec86c06320bc5af1b39a77f"' \
  'SOURCE_RUN_ID: "32733258088"' \
  'SOURCE_BUILD_JOB_ID: "97450107488"' \
  'SOURCE_ATTEST_JOB_ID: "97456266494"' \
  'SOURCE_ARTIFACT_ID: "9522912673"' \
  'SOURCE_ARTIFACT_ARCHIVE_DIGEST: "sha256:048a291a23288c083790aa1881119a70d4d4efb53b95b7d181c979a86cae2d23"' \
  'PRIOR_RUN_ID: "32740154322"' \
  'PRIOR_PREFLIGHT_JOB_ID: "97472417477"' \
  'PRIOR_DRAFT_JOB_ID: "97473253702"' \
  'PRIOR_PUBLISH_JOB_ID: "97473312024"' \
  'PRIOR_CLEANUP_JOB_ID: "97473384617"' \
  'PRIOR_FINALIZE_JOB_ID: "97473386091"' \
  'PRIOR_VERIFY_FINAL_JOB_ID: "97473386526"' \
  'PRIOR_MARKER: "provider-ripgrep-recovery-run:32740154322:1"' \
  'DRAFT_RELEASE_ID: "375773607"' \
  'PACKAGE_VERSION_ID: "1166057577"' \
  'MANIFEST_DIGEST: "sha256:6e2e29d13541c8d5e16e4fcf238d429c1f7f7db6db1d7b2e35aec3ff3ee61142"' \
  'COMPONENT_DIGEST: "sha256:27dfe89eafafca7039e9191e3211ce05ebe7e650b309905a294ba937d130aa0b"' \
  'CHECKSUM_DIGEST: "sha256:cdb323c2ec550e1d463d19091d3ca2512ac878e7f02a293195d1297daeccce5c"' \
  'run-id: "32733258088"' \
  '--source-digest "$SOURCE_SHA"' \
  '--source-ref "refs/tags/$TAG"' \
  'dist/ripgrep-provider.wasm' \
  '"org.opencontainers.image.title":"dist/ripgrep-provider.wasm"' \
  'ripgrep.search' \
  'provider-ripgrep/versions' \
  'Complete distribution license bundle' \
  'This PATCH is the residual finalizer' \
  'anonymous read-only verification after residual finalization'; do
  grep -Fq -- "$required" "$finalizer" || {
    echo "error: residual finalizer omits required exact-state gate: $required" >&2
    exit 1
  }
done

python3 - "$finalizer" <<'PY'
import pathlib
import re
import sys
text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
finalize = text.index("  finalize:")
pre_patch = text.index("Last read-only interlock, then one complete final PATCH")
patch_comment = text.index("# This PATCH is the residual finalizer's only remote mutation.")
post = text.index("  verify_final:")
if not finalize < pre_patch < patch_comment < post:
    raise SystemExit("error: residual read gates/PATCH/post-verification ordering drifted")
if text.count("gh api --method PATCH") != 1:
    raise SystemExit("error: residual finalizer must contain exactly one remote PATCH")
if text.count("contents: write") != 1:
    raise SystemExit("error: only the finalization job may have writable contents permission")
if "packages: write" in text:
    raise SystemExit("error: residual finalizer must not have package write permission")
if text.count("actions/download-artifact") != 2:
    raise SystemExit("error: both jobs must redownload only the fixed source artifact")
if text.count('run-id: "32733258088"') != 2:
    raise SystemExit("error: source artifact downloads are not fixed to the source run")
if "actions/upload-artifact" in text:
    raise SystemExit("error: residual finalizer must never upload substitute bytes")
if "verify-oci-manifest.py" in text:
    raise SystemExit("error: residual dist/ title must not weaken or bypass the normal verifier")
if re.search(r"cargo (?:build|test|check)|prepare-release-assets|reproducible-build", text):
    raise SystemExit("error: residual finalizer must not rebuild provider bytes")
for forbidden in (
    "gh release create", "gh release delete", "gh release upload", "gh release edit",
    "--method DELETE", "--method POST", "--method PUT", "docker/login-action",
    'oras-bin" push', '"$RUNNER_TEMP/oras" push', "cleanup_failed",
):
    if forbidden in text:
        raise SystemExit(f"error: residual finalizer contains forbidden mutation/layout {forbidden!r}")
if re.search(r"gh api\s+--method\s+(?!PATCH)", text):
    raise SystemExit("error: residual finalizer contains a non-PATCH mutating API method")
patch_block = text[pre_patch:post]
for field in ('body: $body', 'name: "v0.1.0"', "prerelease: false", "draft: false"):
    if field not in patch_block:
        raise SystemExit(f"error: one final PATCH omits {field}")
post_text = text[post:]
for mutation in ("--method PATCH", "--method DELETE", "gh release ", ' push "$ref"'):
    if mutation in post_text:
        raise SystemExit(f"error: post-finalization verification can mutate via {mutation}")
if text.count('"dist/ripgrep-provider.wasm"') < 3:
    raise SystemExit("error: exact residual layer title is not repeatedly pinned")
if text.count("sha256:$COMPONENT_SHA256") != 0:
    raise SystemExit("error: finalizer should use its fully pinned COMPONENT_DIGEST")
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
"$verifier" "$temporary/manifest.json" "$temporary/ripgrep-provider.wasm" \
  1:1 "$(printf 'a%.0s' {1..40})"
python3 - "$temporary/manifest.json" <<'PY'
import json
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
manifest = json.loads(path.read_text(encoding="utf-8"))
manifest["layers"][0]["annotations"]["org.opencontainers.image.title"] = \
    "dist/ripgrep-provider.wasm"
path.write_text(json.dumps(manifest), encoding="utf-8")
PY
if "$verifier" "$temporary/manifest.json" "$temporary/ripgrep-provider.wasm" \
  1:1 "$(printf 'a%.0s' {1..40})" >/dev/null 2>&1; then
  echo 'error: normal OCI verifier was globally weakened to accept a path-bearing title' >&2
  exit 1
fi
python3 - "$temporary/manifest.json" <<'PY'
import json
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
manifest = json.loads(path.read_text(encoding="utf-8"))
manifest["layers"][0]["annotations"]["org.opencontainers.image.title"] = \
    "ripgrep-provider.wasm"
manifest["annotations"]["org.opencontainers.image.licenses"] = "MIT OR Apache-2.0"
path.write_text(json.dumps(manifest), encoding="utf-8")
PY
if "$verifier" "$temporary/manifest.json" "$temporary/ripgrep-provider.wasm" \
  1:1 "$(printf 'a%.0s' {1..40})" >/dev/null 2>&1; then
  echo 'error: normal OCI verifier accepted an incomplete binary license expression' >&2
  exit 1
fi

printf 'workflow pins, release push context, exact residual gates, sole PATCH, and strict normal verifier passed\n'
