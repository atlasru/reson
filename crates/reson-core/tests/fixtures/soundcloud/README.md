The public profile and likes-page responses were recorded on 10 October 2026
from SoundCloud's public web client API. Profiles: `scottbuckley` and
`monstercat`. The likes collection is Scott Buckley's.

Only fields consumed by Reson's metadata parsers are retained. Public client
identifiers and streaming/transcoding URLs are omitted. The first page has 46
tracks and a next cursor; the second page is empty with a null cursor. This
guards against stopping pagination merely because a page contains fewer than
the requested 100 tracks.

`likes.json` is a separate fault fixture with a deleted entry and a blocked
track. Mock providers generate the larger multi-page and concurrent scenarios.
All these tests run offline.
