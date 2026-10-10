# Reson v0.2.0 validation

All desktop captures come from the standalone, optimized Tauri application,
with its normal React/core IPC and native libmpv engine. There is no Vite
server, injected demonstration collection, embedded SoundCloud player or
simulated import in these checks. Public service checks were run on
10 October 2026; deterministic CI tests use recorded and explicit fault fixtures.

## Automated checks

39 Rust tests and 16 frontend tests pass, alongside Rust formatting and clippy
with warnings denied, ESLint, TypeScript checking and the production frontend
build. Windows CI also checks and tests the full Tauri workspace.

- Public profile normalization, URL/domain/path rejection, numeric profile
  resolution, recorded real profile responses and malformed cursor responses.
- Three-page import of 207 tracks, following cursors even after a short page;
  empty versus inaccessible collections, deleted entries and unavailable metadata.
- Rate limiting, cancellation during profile resolution, an in-flight page
  and a 1,800-second cooldown. Cancellation checks complete within 500 ms.
- Partial failure recovery, repeated-cursor reporting, reimport idempotency,
  incremental refresh, simultaneous sources and alias/ownership conflicts.
- SQLite reopen, v0.1 schema migration, interrupted job recovery, provider-scoped
  identifiers, track dismissal/deletion and both source removal modes. A
  regression check prevents an already resolving job from recreating a source
  after cancellation/removal.
- Navigation, search, dialog submission/progress/cancellation/partial errors,
  focus trapping/restoration and Escape when a focused input becomes disabled.
  Track selection, double-click/keyboard playback, context-menu queue actions,
  shortcuts, details and provenance. A 12,000-track fixture renders a bounded
  visible window instead of thousands of DOM rows.
- Existing playback race handling, queue/shuffle/repeat behavior, stream refresh,
  outages, storage configuration, artwork eviction and settings volume preservation.

Fixtures are documented in
[`crates/reson-core/tests/fixtures/soundcloud/README.md`](../crates/reson-core/tests/fixtures/soundcloud/README.md).
Ordinary tests never require live SoundCloud access.

## Live import and persistence

The real public SoundCloud web API exposed Scott Buckley's 46 liked tracks.
The first page contained 46 entries and a cursor; a second, empty page supplied
the terminal `next_href: null`. Both pages were requested. This deliberately
checks that a short first page does not stop pagination.

The import manager and the production desktop UI both saved all 46 tracks in
SQLite without authentication. The UI showed profile identity, actual saved,
duplicate and failure counters, and the tracks appeared immediately in Liked
Tracks. Refresh fetched both pages again: 0 new saves, 46 duplicates, 0 failures.
An actual process close/relaunch retained the source and all imported tracks.
Double-clicking an imported track produced native playback through the normal
player; imports never alter remote SoundCloud likes.

These observations describe the public collection returned during the test,
not a guarantee that SoundCloud exposes every user's likes. Inaccessible
responses and partial imports are reported distinctly and preserve committed pages.

## Production desktop interaction and visual review

The final Linux production run passed 54 checks. The production workflow
captures and inspects these real views:

1. Home and real SoundCloud search results.
2. Artist and track details.
3. Active native playback and the queue.
4. Library and the import dialog.
5. Completed import, imported likes, sources and imported-track playback.
6. Settings, plus Settings and Liked Tracks at 900 × 620.

The 1366 × 768 layout and the smaller window fit the persistent playback bar
without clipped controls or horizontal overflow. The small track layout retains
artist names below titles. Visual review removed unreadable native select
backgrounds and the remaining placeholder account panel. Artwork, compact row
alignment, menu placement, text overflow and source provenance were reviewed
from actual desktop captures. README images are selected raw production captures.

Native checks exercise Ctrl K search focus, Back/search restoration, right-click
Play Next/Add to Queue, queue reorder/removal, Escape, Space pause/resume, seek,
volume, shuffle/repeat, real import/refresh and double-clicking a saved import.
Changing settings preserves playback volume. Restart retains both imports and
settings. The report records each successful assertion in `checks.json`.

The Linux desktop run uses WebKitGTK with Xvfb and a PulseAudio null sink.
A six-second output-device monitor recorded 575,026 float PCM samples,
RMS 0.019403 and peak 0.184216, confirming that actual decoded sound reached the
output device. Temporary PCM files were deleted. This does not test physical
speakers. The Linux validation build disables LTO to accommodate the available
linker/sysroot; Windows production builds retain the repository's ThinLTO setting.

## Windows packaging and release validation

[Windows run 38077582767](https://github.com/atlasru/reson/actions/runs/38077582767)
and the matching pull-request run passed all frontend, core and Windows jobs.
The Windows production desktop report records 56 successful checks and 15
actual WebView2 captures, including live import, refresh and process restart.
The final release workflow repeats these checks before publishing.

GitHub Actions builds an x64 NSIS installer and a portable ZIP containing
`Reson.exe`, the pinned LGPL libmpv DLL and all required license notices.
It creates SHA256SUMS.txt for both files, validates live SoundCloud/native PCM
from the portable executable, performs a fresh silent installation and repeats
native decoding from the installed executable. Both probes inspect 4,194,304
float PCM samples (RMS 0.065142, peak 0.714613).

The installed desktop is then started normally. CI waits for its native window
and successful frontend bootstrap, checks native audio initialization, and
requires a normal process exit when the window is closed.

The full production UI script attaches EdgeDriver to the actual release app's
WebView2, exercises the workflow above, captures its rendered views and restarts
the native process. The driver version matches the installed WebView2 runtime.
On elevated Windows runners, temporary HKLM policy values scoped to `Reson.exe`
enable the test connection and an isolated WebView profile. The workflow restores
these values afterward. No test connection or policy is installed with the app.

The headless Windows UI check explicitly selects libmpv's native null output
through the normal settings IPC. Real HLS decoding and playback timing remain
active. Audible PCM energy is checked separately by both Windows executable
probes and by the Linux output-device monitor; a UI timer alone is insufficient.

Release publication requires successful frontend, core and Windows jobs and
verifies the SHA-256 manifest before publishing. A failed run is never described
as a validated release. The current run and screenshot artifacts are linked
from the pull request and release.

## Repeating the checks

Run the normal format/lint/typecheck/test/build commands in the README.
For a live, persistent-library smoke test:

```sh
cargo run -p reson-core --example likes_probe -- scottbuckley
```

For production desktop UI checks, start `tauri-driver` and run:

```sh
python scripts/validate-redesign.py --application /absolute/path/to/reson --output ./screenshots
```

On Linux, direct `WebKitWebDriver` can also be used with
`--native-webkit-driver --driver-url http://127.0.0.1:4445`.
A configured PulseAudio monitor enables `--capture-device reson_test.monitor`.
Headless Xvfb must remain alive across app exits (`-noreset`).
Windows Actions supplies the WebView2 connection setup and driver automatically.
On a headless Windows host with no sound device, pass `--audio-device null`.

## Environment limits

Physical Windows speakers, device removal, hardware media keys, media-overlay
rendering and live Discord presence require an interactive hardware desktop and,
for Discord, a registered application. Those hardware/service observations are
not claimed by the headless tests. Public imports require no account; SoundCloud
OAuth and remote account modifications remain outside this release.
