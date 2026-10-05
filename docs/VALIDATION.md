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

## Automated verification

Core tests exercise malformed/oversized provider input, cancellation, rate
cooldowns, large URNs, normalized identities, multi-source preservation,
unavailable content, source refresh, outage behavior, loading/pause races,
queue ordering/shuffle/repeat, configuration, artwork eviction and SQLite
migration/reopen. Frontend tests verify seek gestures and real control IPC.

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
