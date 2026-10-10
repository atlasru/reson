# Reson v0.2.0

A compact desktop interface and a persistent library of public SoundCloud likes.

- Rebuilt navigation with separate Library, Liked Tracks and Playlists pages; collapsible and resizable sidebar with saved playlists.
- Compact 48 px virtual track rows, selection, double-click playback, keyboard navigation and right-click menus. Track details and artwork-focused artist/playlist pages remain available.
- One persistent playback bar. Native libmpv playback, seeking, volume, shuffle/repeat, queue editing, Windows media controls and tray behavior are preserved.
- Import any publicly accessible SoundCloud profile's liked tracks by username or HTTPS profile URL. No account required. Follow every `next_href` until the public collection ends.
- Live progress, duplicate/failed/unavailable counters, cancellable requests and rate-limit waits; saved pages survive cancellation and failure.
- SQLite migration preserves v0.1.0 data. Stable provider-scoped identities, multiple import sources, incremental refresh, source filters and provenance.
- Removing a track from Reson likes prevents refresh from restoring it. Removing a source can retain its tracks as local likes or remove only its own imported memberships; independent local likes and other sources remain.

Public imports are local snapshots. They do not modify anybody's remote SoundCloud likes. Hidden/private likes are inaccessible. Unavailable tracks retain metadata and stay marked unavailable. A partial response is reported as partial, never as a completed full import. Refresh keeps previously imported tracks that disappear remotely.

Windows 10/11 x64. Installer and portable ZIP include the existing pinned LGPL libmpv build and license notices. SHA256SUMS.txt covers both downloads. MIT application license retained.
