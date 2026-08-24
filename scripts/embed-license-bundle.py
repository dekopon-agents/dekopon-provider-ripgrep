#!/usr/bin/env python3
"""Embed and verify the exact distribution-license bundle in a Wasm custom section."""

from __future__ import annotations

import pathlib
import sys

SECTION_NAME = b"dekopon.third-party-notices"
BUNDLE_HEADER = b"DEKOPON DISTRIBUTION LICENSE BUNDLE v1\n"
BUNDLE_FILES = (
    "THIRD_PARTY_NOTICES.md",
    "licenses/encoding_rs-0.8.35-LICENSE-WHATWG",
    "LICENSE-MIT",
    "LICENSE-APACHE",
)


def fail(message: str) -> None:
    raise SystemExit(f"error: {message}")


def encode_uleb(value: int) -> bytes:
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            byte |= 0x80
        encoded.append(byte)
        if not value:
            return bytes(encoded)


def decode_uleb(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    for _ in range(5):
        if offset >= len(data):
            fail("truncated Wasm LEB128")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7
    fail("invalid Wasm u32 LEB128")


def license_bundle(root: pathlib.Path) -> bytes:
    chunks = [BUNDLE_HEADER]
    for relative in BUNDLE_FILES:
        path = root / relative
        if not path.is_file():
            fail(f"missing bundle input {path}")
        contents = path.read_bytes()
        if not contents.endswith(b"\n"):
            fail(f"bundle input lacks final LF: {path}")
        chunks.extend(
            (
                f"\n===== BEGIN {relative} =====\n".encode(),
                contents,
                f"===== END {relative} =====\n".encode(),
            )
        )
    bundle = b"".join(chunks)
    required = (
        b"(MIT OR Apache-2.0) AND BSD-3-Clause",
        b"Copyright \xc2\xa9 WHATWG (Apple, Google, Mozilla, Microsoft).",
        b"Redistributions in binary form must reproduce",
    )
    for phrase in required:
        if phrase not in bundle:
            fail(f"license bundle omits required phrase {phrase!r}")
    return bundle


def custom_sections(data: bytes) -> list[tuple[bytes, bytes]]:
    if len(data) < 8 or data[:4] != b"\0asm":
        fail("input is not a WebAssembly binary")
    sections: list[tuple[bytes, bytes]] = []
    offset = 8
    while offset < len(data):
        section_id = data[offset]
        offset += 1
        size, offset = decode_uleb(data, offset)
        end = offset + size
        if end > len(data):
            fail("truncated WebAssembly section")
        if section_id == 0:
            name_len, payload_offset = decode_uleb(data, offset)
            name_end = payload_offset + name_len
            if name_end > end:
                fail("truncated WebAssembly custom-section name")
            sections.append((data[payload_offset:name_end], data[name_end:end]))
        offset = end
    if offset != len(data):
        fail("invalid WebAssembly section framing")
    return sections


def verify_component(path: pathlib.Path, root: pathlib.Path) -> None:
    expected = license_bundle(root)
    payloads = [
        payload for name, payload in custom_sections(path.read_bytes()) if name == SECTION_NAME
    ]
    if len(payloads) != 1:
        fail(f"expected one {SECTION_NAME.decode()} custom section, found {len(payloads)}")
    if payloads[0] != expected:
        fail("embedded distribution-license bundle is not byte-exact")


def embed_component(path: pathlib.Path, root: pathlib.Path) -> None:
    data = path.read_bytes()
    if any(name == SECTION_NAME for name, _payload in custom_sections(data)):
        fail(f"component already contains {SECTION_NAME.decode()}")
    bundle = license_bundle(root)
    named_payload = encode_uleb(len(SECTION_NAME)) + SECTION_NAME + bundle
    path.write_bytes(data + b"\0" + encode_uleb(len(named_payload)) + named_payload)
    verify_component(path, root)


def main() -> None:
    if len(sys.argv) != 4 or sys.argv[1] not in {"embed", "verify", "write", "verify-text"}:
        fail("usage: embed-license-bundle.py {embed|verify|write|verify-text} PATH REPO_ROOT")
    command = sys.argv[1]
    path = pathlib.Path(sys.argv[2])
    root = pathlib.Path(sys.argv[3]).resolve()
    if command == "embed":
        embed_component(path, root)
    elif command == "verify":
        verify_component(path, root)
    elif command == "write":
        path.write_bytes(license_bundle(root))
    else:
        contents = path.read_bytes()
        expected = license_bundle(root)
        if contents.count(expected) != 1:
            fail("release documentation must contain the exact license bundle once")


if __name__ == "__main__":
    main()
