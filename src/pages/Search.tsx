import { useEffect, useRef, useState } from "react";
import { Search as SearchIcon, X, ArrowDown } from "lucide-react";
import { call, useProviders, useSelectedProvider } from "../stores/core";
import type {
  Route,
  SearchResults,
  Track,
  Artist,
  Playlist,
} from "../stores/types";
import { TrackList } from "../components/TrackList";
import { ArtistCards, PlaylistCards } from "../components/Cards";
import { Empty, Failure, Skeleton } from "../components/States";
export function Search({ navigate }: { navigate: (r: Route) => void }) {
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<"tracks" | "artists" | "playlists">("tracks");
  const [data, setData] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [offset, setOffset] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const active = useRef("");
  const available = useProviders();
  const selectedSource = useSelectedProvider();
  const provider =
    selectedSource ??
    available.find((p) => p.capabilities.includes("search"))?.id ??
    "soundcloud";
  useEffect(
    () => () => {
      if (active.current)
        void call("cancel_request", { requestId: active.current }).catch(
          () => {},
        );
    },
    [],
  );
  useEffect(() => {
    input.current?.focus();
    const handler = () => input.current?.focus();
    window.addEventListener("focus-search", handler);
    return () => window.removeEventListener("focus-search", handler);
  }, []);
  useEffect(() => {
    const q = query.trim();
    setError("");
    setData(null);
    setOffset(0);
    if (!q) {
      setLoading(false);
      return;
    }
    setLoading(true);
    const id = crypto.randomUUID();
    active.current = id;
    const timer = window.setTimeout(() => {
      if (q.startsWith("https://")) {
        void call<{ kind: string; entity: Track | Artist | Playlist }>(
          "resolve_url",
          { provider, url: q },
        )
          .then((r) => {
            if (active.current !== id) return;
            setLoading(false);
            if (r.kind === "track")
              navigate({ page: "track", track: r.entity as Track });
            else if (r.kind === "playlist")
              navigate({ page: "playlist", playlist: r.entity as Playlist });
            else {
              const a = r.entity as Artist;
              const ref = a.references[0];
              if (ref)
                navigate({
                  page: "artist",
                  provider: ref.provider,
                  id: ref.provider_id,
                });
            }
          })
          .catch((e) => {
            if (active.current === id) {
              setError(String(e));
              setLoading(false);
            }
          });
        return;
      }
      void call<SearchResults>("search", {
        provider,
        query: q,
        offset: 0,
        requestId: id,
      })
        .then((r) => {
          if (active.current === id) {
            setData(r);
            setLoading(false);
          }
        })
        .catch((e) => {
          if (active.current === id) {
            setError(String(e));
            setLoading(false);
          }
        });
    }, 350);
    return () => {
      window.clearTimeout(timer);
      if (active.current === id) active.current = "";
      void call("cancel_request", { requestId: id }).catch(() => {});
    };
  }, [query, provider, retry, navigate]);
  const more = async () => {
    if (loading) return;
    setLoading(true);
    const id = crypto.randomUUID();
    active.current = id;
    try {
      const next = offset + 30;
      const r = await call<SearchResults>("search", {
        provider,
        query,
        offset: next,
        requestId: id,
      });
      setData((old) =>
        old
          ? {
              tracks: [...old.tracks, ...r.tracks],
              artists: [...old.artists, ...r.artists],
              playlists: [...old.playlists, ...r.playlists],
              has_more: r.has_more,
            }
          : r,
      );
      setOffset(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };
  return (
    <div className="page">
      <header className="page-title">
        <div>
          <span className="eyebrow">Explore a source</span>
          <h1>Search</h1>
        </div>
      </header>
      <div className="search-box">
        <SearchIcon size={20} />
        <input
          ref={input}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Tracks, artists, playlists or a SoundCloud link"
          aria-label="Search music"
        />
        {query && (
          <button
            className="icon"
            title="Clear search"
            onClick={() => setQuery("")}
          >
            <X size={18} />
          </button>
        )}
        <kbd>Ctrl K</kbd>
      </div>
      <div className="tabs">
        {(["tracks", "artists", "playlists"] as const).map((t) => (
          <button
            key={t}
            className={tab === t ? "selected" : ""}
            onClick={() => setTab(t)}
          >
            {t[0].toUpperCase() + t.slice(1)}
            {data && <span>{data[t].length}</span>}
          </button>
        ))}
      </div>
      {loading && !data ? (
        <Skeleton />
      ) : error && !data ? (
        <Failure message={error} retry={() => setRetry((n) => n + 1)} />
      ) : !data ? (
        <Empty
          title="Find your next listen"
          detail="Search SoundCloud’s public catalog. No account required."
        />
      ) : tab === "tracks" ? (
        <TrackList tracks={data.tracks} navigate={navigate} />
      ) : (
        <div className="page-scroll">
          {tab === "artists" ? (
            <ArtistCards artists={data.artists} navigate={navigate} />
          ) : (
            <PlaylistCards playlists={data.playlists} navigate={navigate} />
          )}
        </div>
      )}
      {data?.has_more && (
        <button
          className="load-more secondary"
          onClick={() => void more()}
          disabled={loading}
        >
          <ArrowDown size={14} />
          {loading ? "Loading…" : "Load more"}
        </button>
      )}
      {error && data && <p className="inline-error">{error}</p>}
    </div>
  );
}
