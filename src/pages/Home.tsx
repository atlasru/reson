import { useEffect, useState } from "react";
import {
  call,
  play,
  selectedProvider,
  useLibrary,
  useProviders,
  useSelectedProvider,
} from "../stores/core";
import type { Route, Track } from "../stores/types";
import { Artwork } from "../components/Artwork";
import { TrackList } from "../components/TrackList";
import { Empty, Failure, Skeleton } from "../components/States";
const catalog = new Map<string, Track[]>();
export function Home({ navigate }: { navigate: (r: Route) => void }) {
  const lib = useLibrary();
  const providers = useProviders();
  const selectedSource = useSelectedProvider();
  const provider = selectedSource ?? providers[0]?.id ?? "";
  const info = providers.find((p) => p.id === provider);
  const [anchor, setAnchor] = useState<Track | null>(lib.recent[0] ?? null);
  const [mode, setMode] = useState("trending");
  const source = anchor?.sources.find((s) => s.provider === provider);
  const id =
    mode === "related" && info?.capabilities.includes("related")
      ? source?.provider_id
      : undefined;
  const key = `${provider}:${id ?? "trending"}`;
  const [tracks, setTracks] = useState<Track[]>(catalog.get(key) ?? []);
  const [loading, setLoading] = useState(!catalog.has(key));
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const canDiscover = !!info?.capabilities.includes(
    id ? "related" : "discovery",
  );
  useEffect(() => {
    let alive = true;
    setError("");
    const cached = catalog.get(key);
    if (cached && retry === 0) {
      setTracks(cached);
      setLoading(false);
      return;
    }
    setLoading(true);
    if (!canDiscover) {
      setLoading(false);
      return;
    }
    void call<Track[]>(id ? "related_tracks" : "discover", {
      provider,
      id: id ?? null,
    })
      .then((rows) => {
        if (alive) {
          catalog.set(key, rows);
          setTracks(rows);
        }
      })
      .catch((e) => {
        if (alive) setError(String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [id, provider, key, canDiscover, retry]);
  return (
    <div className="page home-page">
      <header className="page-title">
        <h1>Home</h1>
        {providers.length > 1 && (
          <select
            aria-label="Music source"
            value={provider}
            onChange={(e) => selectedProvider.set(e.target.value)}
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.display_name}
              </option>
            ))}
          </select>
        )}
      </header>
      {lib.recent.length > 0 && (
        <section className="recent-strip">
          <h2>Recently played</h2>
          <div>
            {lib.recent.slice(0, 4).map((t) => (
              <button
                className="recent-track"
                key={t.internal_id}
                onClick={() => play([t.internal_id])}
              >
                <Artwork url={t.artwork} />
                <span>
                  <strong>{t.title}</strong>
                  <small>{t.artists.map((a) => a.name).join(", ")}</small>
                </span>
              </button>
            ))}
          </div>
        </section>
      )}
      <div className="tabs">
        <button
          className={mode === "trending" ? "selected" : ""}
          onClick={() => setMode("trending")}
        >
          Trending
        </button>
        <button
          className={mode === "related" ? "selected" : ""}
          disabled={
            !lib.recent.length || !info?.capabilities.includes("related")
          }
          onClick={() => {
            setAnchor(lib.recent[0] ?? null);
            setMode("related");
          }}
        >
          For you
        </button>
      </div>
      <div className="section-title">
        <h2>
          {id ? `More like ${anchor?.title}` : (info?.display_name ?? provider)}
        </h2>
      </div>
      {!canDiscover ? (
        <Empty
          title="Explore this source"
          detail="Search to find music."
          action={
            <button
              className="secondary"
              onClick={() => navigate({ page: "search" })}
            >
              Search music
            </button>
          }
        />
      ) : loading ? (
        <Skeleton />
      ) : error ? (
        <Failure message={error} retry={() => setRetry((n) => n + 1)} />
      ) : (
        <TrackList
          tracks={tracks}
          navigate={navigate}
          listKey={`home:${key}`}
        />
      )}
    </div>
  );
}
