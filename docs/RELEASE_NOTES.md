# Reson v0.1.0

First Windows x64 release of Reson, a local-first music player with a native
provider-neutral Rust playback engine.

- Real SoundCloud guest search, tracks, users, playlists, public discovery and
  related tracks. AAC HLS playback through bundled LGPL libmpv.
- Pause/resume, seek, volume/output devices, editable queue, shuffle and repeat.
- Persistent favorites, history, local playlists, queue and settings.
- Desktop keyboard controls and Windows media session integration.
- Bounded artwork cache, clear errors and local diagnostics.
- Optional local Discord Rich Presence with a configured application ID.

Download `Reson_0.1.0_x64-setup.exe` to install, or extract all files from
`Reson_0.1.0_windows_x64_portable.zip` and run `Reson.exe`. Both use the normal
per-user app data directory; portable refers to distribution, not isolated
profile storage. Windows 10/11 x64 and WebView2 are required; the installer
bootstraps WebView2 if missing. Builds are unsigned.

SoundCloud access uses its undocumented public web interface and may be
affected by service/region changes. Login, private/premium playback, SoundCloud
likes/following, Reson accounts/synchronization and telemetry collection are
not included. No account is required for public playback.

Application code is MIT. Bundled native components retain their LGPL/other
licenses and notices. See `docs/SOUNDCLOUD.md`, `docs/ARCHITECTURE.md` and the
repository README for details and validation evidence.
