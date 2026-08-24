# Third-party notices

`dekopon-ripgrep-provider` project source is licensed under **MIT OR Apache-2.0**. The release
component statically links permissively licensed Rust dependencies. `Cargo.lock is the authority`
for every exact transitive version and crates.io checksum; `cargo deny check licenses` is the
machine-enforced license inventory.

The binary distribution license expression is **(MIT OR Apache-2.0) AND BSD-3-Clause**. The
release component embeds this file together with `LICENSE-MIT` and `LICENSE-APACHE` in the
`dekopon.third-party-notices` Wasm custom section. The immutable GitHub release reproduces the same
complete bundle in its release documentation. This keeps the GitHub release at exactly two assets
and the OCI artifact at exactly one byte-identical Wasm layer while carrying the notices in both
distribution channels.

## Search implementation

The search implementation uses the official ripgrep library crates by Andrew Gallant and
contributors:

| Crate | Exact version | License |
|---|---:|---|
| `grep-matcher` | 0.1.9 | Unlicense OR MIT |
| `grep-regex` | 0.1.14 | Unlicense OR MIT |
| `grep-searcher` | 0.1.17 | Unlicense OR MIT |

Their upstream repository is <https://github.com/BurntSushi/ripgrep>. The graph also contains
`aho-corasick`, `memchr`, `regex-automata`, and `regex-syntax` from the Rust regex ecosystem under
MIT/Apache-2.0 or Unlicense/MIT terms.

`grep-searcher` unconditionally depends on `encoding_rs 0.8.35`, `encoding_rs_io 0.1.8`, and
`memmap2 0.9.11`. Those packages are therefore present in the linked graph, but v0.1.0 explicitly
configures no encoding, disables BOM sniffing, selects `MmapChoice::never()`, and calls only
`search_slice` over already decoded caller-owned UTF-8. No file, mmap, or transcoding API is
invoked. `encoding_rs` is licensed under `(Apache-2.0 OR MIT) AND BSD-3-Clause`; its mandatory
WHATWG notice is reproduced verbatim below.

## Dekopon component interface

The component uses `dekopon-provider-sdk 0.11.1`, with `dekopon-core 0.11.1` and
`dekopon-capability 0.11.1`, under MIT OR Apache-2.0. Its generated component bindings use
`wit-bindgen 0.44.0` and the Wasm/WIT 0.236.1 toolchain crates, under Apache-2.0 WITH
LLVM-exception OR Apache-2.0 OR MIT. The release component has zero imports.

`dekopon-provider-sdk-testkit 0.11.1`, Tokio, Wasmtime, HTTP-host, and storage-host packages occur
only in native development/test resolution. They are not linked into `ripgrep-provider.wasm`; the
component-target tree and decoded WIT gates enforce that distinction.

## Serialization and supporting crates

The component directly pins `serde 1.0.229` and `serde_json 1.0.151` under MIT OR Apache-2.0.
Other exact transitive packages are permissively licensed under the allowlist in `deny.toml`,
including MIT, Apache-2.0, BSD-3-Clause, Unicode-3.0, Unlicense, and Zlib terms. The complete
project MIT and Apache-2.0 texts are adjacent files in source and are included verbatim in the
embedded/release-documentation bundle.

## Verbatim notices

### Rust regex and ripgrep crates — MIT option

The following is the common MIT notice carried by `grep-matcher 0.1.9`, `grep-regex 0.1.14`,
`grep-searcher 0.1.17`, `aho-corasick 1.1.5`, and `memchr 2.8.3`:

> The MIT License (MIT)
>
> Copyright (c) 2015 Andrew Gallant
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

### encoding_rs 0.8.35 — copyright statement

> encoding_rs is copyright Mozilla Foundation.
>
> Licensed under the Apache License, Version 2.0
> <LICENSE-APACHE or
> https://www.apache.org/licenses/LICENSE-2.0> or the MIT
> license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
> at your option. All files in the project carrying such
> notice may not be copied, modified, or distributed except
> according to those terms.
>
> This crate includes data derived from the data files supplied
> with the WHATWG Encoding Standard, which, when incorporated into
> source code, are licensed under the BSD 3-Clause License
> <LICENSE-WHATWG>.
>
> Test code within encoding_rs is dedicated to the Public Domain when so
> designated (see the individual files for PD/CC0-dedicated sections).

### encoding_rs 0.8.35 — MIT option

> Copyright Mozilla Foundation
>
> Permission is hereby granted, free of charge, to any
> person obtaining a copy of this software and associated
> documentation files (the "Software"), to deal in the
> Software without restriction, including without
> limitation the rights to use, copy, modify, merge,
> publish, distribute, sublicense, and/or sell copies of
> the Software, and to permit persons to whom the Software
> is furnished to do so, subject to the following
> conditions:
>
> The above copyright notice and this permission notice
> shall be included in all copies or substantial portions
> of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
> ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
> TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
> PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
> SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
> CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
> OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
> IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
> DEALINGS IN THE SOFTWARE.

### encoding_rs 0.8.35 — LICENSE-WHATWG (BSD-3-Clause)

The following notice is reproduced verbatim from `encoding_rs 0.8.35/LICENSE-WHATWG`:

> Copyright © WHATWG (Apple, Google, Mozilla, Microsoft).
>
> Redistribution and use in source and binary forms, with or without
> modification, are permitted provided that the following conditions are met:
>
> 1. Redistributions of source code must retain the above copyright notice, this
>    list of conditions and the following disclaimer.
>
> 2. Redistributions in binary form must reproduce the above copyright notice,
>    this list of conditions and the following disclaimer in the documentation
>    and/or other materials provided with the distribution.
>
> 3. Neither the name of the copyright holder nor the names of its
>    contributors may be used to endorse or promote products derived from
>    this software without specific prior written permission.
>
> THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
> AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
> IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
> DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
> FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
> DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
> SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
> CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
> OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
> OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

### foldhash 0.1.5 — Zlib

> Copyright (c) 2024 Orson Peters
>
> This software is provided 'as-is', without any express or implied warranty. In
> no event will the authors be held liable for any damages arising from the use of
> this software.
>
> Permission is granted to anyone to use this software for any purpose, including
> commercial applications, and to alter it and redistribute it freely, subject to
> the following restrictions:
>
> 1. The origin of this software must not be misrepresented; you must not claim
>    that you wrote the original software. If you use this software in a product,
>    an acknowledgment in the product documentation would be appreciated but is
>    not required.
>
> 2. Altered source versions must be plainly marked as such, and must not be
>    misrepresented as being the original software.
>
> 3. This notice may not be removed or altered from any source distribution.

### Unicode data — Unicode-3.0

The following notice is carried by `unicode-ident 1.0.24`, which participates in the pinned
component build graph:

> UNICODE LICENSE V3
>
> COPYRIGHT AND PERMISSION NOTICE
>
> Copyright © 1991-2023 Unicode, Inc.
>
> NOTICE TO USER: Carefully read the following legal agreement. BY
> DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
> SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
> TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
> DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.
>
> Permission is hereby granted, free of charge, to any person obtaining a
> copy of data files and any associated documentation (the "Data Files") or
> software and any associated documentation (the "Software") to deal in the
> Data Files or Software without restriction, including without limitation
> the rights to use, copy, modify, merge, publish, distribute, and/or sell
> copies of the Data Files or Software, and to permit persons to whom the
> Data Files or Software are furnished to do so, provided that either (a)
> this copyright and permission notice appear with all copies of the Data
> Files or Software, or (b) this copyright and permission notice appear in
> associated Documentation.
>
> THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
> KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
> MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
> THIRD PARTY RIGHTS.
>
> IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
> BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
> OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
> WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
> ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
> FILES OR SOFTWARE.
>
> Except as contained in this notice, the name of a copyright holder shall
> not be used in advertising or otherwise to promote the sale, use or other
> dealings in these Data Files or Software without prior written
> authorization of the copyright holder.

## Reproducing the inventory

Package-specific source files remain available in the exact crates.io sources identified by
`Cargo.lock`. To reproduce and verify the shipped inventory and embedded bundle:

```console
cargo deny list
cargo deny check licenses advisories bans sources
cargo tree --locked --target wasm32-unknown-unknown --edges normal,build
./scripts/check-third-party-notices.sh Cargo.lock THIRD_PARTY_NOTICES.md
./scripts/embed-license-bundle.py verify ripgrep-provider.wasm .
```
