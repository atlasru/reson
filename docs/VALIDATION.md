# Validation evidence

Validation uses real public SoundCloud content, not a mock catalog or embedded
web player. Test audio output is deleted after inspection.

## Native desktop on Linux, 5 October 2026

The actual Tauri application was launched with an empty profile under
WebKitGTK/Xvfb. WebDriver operated the rendered React UI and inspected the
authoritative Rust snapshots over the normal application IPC.

- Search `Scott Buckley`: 30 real tracks, 30 artists and 30 playlists.
- Select `Pigstep — Scott Buckley`: fresh AAC HLS source resolved and native
  libmpv playback advanced in real time.
- PulseAudio output-device monitor: 575,182 float PCM samples in a six-second
  capture; RMS 0.022071, peak 0.161145. This confirms audio reached an output
  device rather than merely updating a timer. The device was a headless null
  sink; physical speakers were unavailable.
- UI pause stopped position at 9,497 ms; resume advanced it. UI HLS seek moved
  to 45,000 ms. UI volume updated core state to 0.3.
- A 30-track queue was edited/reordered/removed; shuffle and every repeat mode
  were toggled in the UI. Next loaded another real track and Previous returned
  to the earlier track.
- The process was closed and relaunched. Queue/current entry, about 30 seconds
  of position, volume, favorites, local playlist and installation UUID persisted.
  State restored paused; Resume refreshed the source and continued playback.
- Space controlled playback after restart. Actual window screenshots are in
  `docs/images/`; their content was not fabricated or edited into a mockup.

Separate native decoding validated 4,194,304 inspected float PCM samples,
RMS 0.065142, peak 0.714613 from the refreshed SoundCloud HLS source.

## Production bundle and Windows

The standalone Linux production bundle was built and run without a Vite/dev
server. Real artist tracks and a hydrated SoundCloud playlist rendered; Back
retained the search query/results. A nonexistent public SoundCloud link
returned a recoverable unavailability error. Playback, controls and queue
editing passed again with 575,098 captured output samples (RMS 0.022084).
A separately launched production process restored the 29-entry queue, local
library, volume 0.3 and position 30,000 ms, then refreshed the source and resumed.

The final production rerun with `scripts/validate-desktop.py` passed all 27
functional checks, including two sessions in the same WebDriver setup. It
captured 569,280 output-device PCM samples (RMS 0.022049, peak 0.160746), verified
that changing the cache setting preserves playback volume, and resumed a fresh
stream after restarting. The README captures come from this production run.

The production bundle was also launched with a deliberately unreachable HTTP
proxy. Real network I/O failed: the player showed a useful error, retained the
queue, and local favorites/playlist edits remained functional. Search failed
gracefully and the application stayed responsive.

[Windows validation run 37285650816](https://github.com/atlasru/reson/actions/runs/37285650816)
completed successfully. Both portable and freshly installed Windows executables
produced 4,194,304 inspected PCM samples, RMS 0.065142, peak 0.714613 from actual
SoundCloud AAC HLS. The release pipeline repeats these checks on the
release commit before publishing its artifacts.

Tracked source and native release output were scanned for hardcoded access
tokens/client secrets, GitHub credential patterns and private-key blocks; no
credentials were found. Transient URLs remain runtime memory only.

## Automated verification

Core tests exercise malformed/oversized provider input, cancellation, rate
cooldowns, large URNs, normalized identities, multi-source preservation,
unavailable content, source refresh, outage behavior, loading/pause races,
queue ordering/shuffle/repeat, configuration, artwork eviction and SQLite
migration/reopen. Frontend tests verify seek gestures and real control IPC.

25 core tests and 3 frontend tests pass. The frontend regression test also
guards volume preservation when subsequent Settings changes are submitted.

To repeat desktop checks, start `tauri-driver` in a fresh profile and run
`python scripts/validate-desktop.py --application /absolute/path/to/reson`.
Linux output-device capture can be included with
`--capture-device reson_test.monitor` and a configured PulseAudio server.
Screenshots/reports go to the selected `--output` directory. The full two-session
check requires a WebDriver/display setup that remains alive across app exits;
headless Xvfb should use `-noreset`.

Windows Actions checks the full workspace, produces x64 installer/portable
builds, validates the portable DLL against live SoundCloud, performs a fresh
silent installation and repeats native decoding from the installed executable.
CI status and artifacts are linked from the release/PR. An in-progress or failed
run is never described as successful.

## Limits of this environment

Windows media-overlay rendering, physical hardware media keys, real speakers,
device removal and live Discord presence need an interactive Windows desktop
and a registered Discord application. Native session/presence integration is
implemented, but those hardware/service checks are not claimed here. No
SoundCloud OAuth or Reson account backend is provisioned.
