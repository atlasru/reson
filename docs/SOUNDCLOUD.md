# SoundCloud integration

Investigated against the live service on 5 October 2026. This is an independent
client, not affiliated with SoundCloud. Public access remains subject to
SoundCloud's terms, geographic restrictions and service changes.

## Official API findings

- [API guide](https://developers.soundcloud.com/docs/api/guide): OAuth 2.1,
  authorization-code flow with PKCE; registered applications are confidential
  clients and require a client secret. Registration requires an eligible
  SoundCloud account. A distributable desktop binary cannot safely keep this
  secret. Public client-credentials tokens also require that secret.
- [Rate limits](https://developers.soundcloud.com/docs/api/rate-limits): the
  documented stream-play limit is 15,000 requests per 24 hours per client ID.
  Client-credentials issuance has separate application and IP limits. These
  documented limits should not be assumed to describe the public web API.
- [Streaming update](https://developers.soundcloud.com/blog/api-streaming-urls/):
  AAC HLS replaces the official progressive MP3/Opus delivery. AAC 160 is
  preferred; AAC 96 is the lower-bandwidth option.
- [URN migration](https://developers.soundcloud.com/blog/urn-num-to-string/):
  entity references use string URNs. IDs must not be truncated to 32 bits.

## What v0.1.0 uses

Guest SoundCloud access uses the **undocumented public web API**
`api-v2.soundcloud.com`. The current public client identifier is discovered
from scripts explicitly linked by SoundCloud's homepage. It is held in memory
for at most one hour, is never compiled into the binary and never appears in
logs. This identifier is public web configuration, not an OAuth client secret
or an account credential.

Search, user tracks/playlists, playlists, public charts, URL resolution and
related tracks are implemented in `crates/reson-core/src/providers/soundcloud/`.
No page calls this API directly. The generic provider interface exposes
explicit capabilities and normalized entities.

Playback fetches fresh track metadata, checks streamability and policy, selects
an available transcoding, then resolves its transient URL using the returned
track authorization. AAC 160 HLS is preferred, followed by AAC 96 and other
available public formats. Legacy progressive delivery is only a fallback when
the live web API still offers it. URLs and track authorizations remain in RAM.
libmpv handles the HLS manifest and segment decoding. A native stream failure
refreshes the source once. A failing track never tears down the queue.

HTTPS endpoints/redirects and stream origins are validated. Responses are
bounded; requests support cancellation, limited concurrency, bounded retries
and 429 cooldowns. Private, deleted, blocked, subscription-only and
non-streamable tracks are not bypassed. Preview tracks are clearly labelled.

**Reliability boundary:** this public web interface can change without notice.
SoundCloud access may fail if it changes, a region blocks playback or a
provider-side limit applies. The application reports the failure and retains
local state. These dependencies are isolated so an official API integration
can replace them without changing playback, queue or pages.

## Authentication

SoundCloud login, likes, following, personal playlists and personalized feeds
are **not implemented**. No cookies, browser sessions or user access tokens are
scraped. Future official OAuth support needs registered credentials and a
deployed token service, PKCE/state validation and OS-backed token storage.
Reson favorites and local playlists are separate, fully functional local data.

## Playback validation

`cargo run -p reson-core --example provider_probe` performs real guest search,
stream resolution and native PCM decoding, checks actual sample energy and
deletes its temporary audio output. A shipped Windows executable supports
`--smoke` for the same noninteractive check. CI executes it in both the
portable and freshly installed builds. These commands require internet access
and libmpv; normal unit tests never contact SoundCloud.
