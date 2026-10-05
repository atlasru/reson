# Third-party components

Reson application code is MIT licensed. Native audio is dynamically loaded from
an unmodified, replaceable **LGPL libmpv** build with its FFmpeg dependencies.

Windows binary: zhongfly/mpv-winbuild, 2026-10-04-c152964208,
`mpv-dev-lgpl-x86_64-20261004-git-c152964208.7z`.
SHA-256: b656b1d3f18f3db619894c22b68023acabd856e9e026b2a8516f30f1a44ee0bc

Source and build recipes:
https://github.com/zhongfly/mpv-winbuild/tree/2026-10-04-c152964208
https://github.com/mpv-player/mpv/commit/c1529642089bfebfc928a1c1664638a7a5d219ba
https://github.com/zhongfly/mpv-winbuild/blob/main/compile-lgpl-libmpv.patch
https://github.com/zhongfly/mpv-winbuild/releases/tag/2026-10-04-c152964208

libmpv LGPL-2.1-or-later; FFmpeg LGPL-3.0-or-later. License texts and upstream
notices are distributed in `licenses/`. Replace `libmpv-2.dll` with an ABI v2
compatible library to use a modified build. No restrictions on reverse
engineering for debugging modifications to the LGPL components.

React (MIT), Tauri (MIT/Apache-2.0), Tokio (MIT), reqwest (MIT/Apache-2.0),
SQLite (public domain), rusqlite (MIT), libloading (ISC), lucide (ISC).
The pinned Cargo.lock/package-lock.json identify all dependencies.

`licenses/DEPENDENCIES.txt` is generated from the locked Rust/npm dependencies
at build time and includes their upstream copyright and license notices.
Workspace license files omitted by some crate packages are supplied alongside
it. The unmodified MPL components' source is available in the locked crates
and upstream repositories, including https://github.com/servo/stylo.
