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
type SearchTab = "tracks" | "artists" | "playlists";
type SearchSession = {
  query: string;
  tab: SearchTab;
  data: SearchResults;
  offset: number;
};
// Navigation memory only; queries are never persisted or sent to a Reson service.
const sessions = new Map<string, SearchSession>();
export function Search({ navigate }: { navigate: (r: Route) => void }) {
  const available = useProviders();
  const selectedSource = useSelectedProvider();
  const provider =
    selectedSource ??
    available.find((p) => p.capabilities.includes("search"))?.id ??
    "";
  const initial = sessions.get(provider);
  const [query, setQuery] = useState(initial?.query ?? "");
  const [tab, setTab] = useState<SearchTab>(initial?.tab ?? "tracks");
  const [data, setData] = useState<SearchResults | null>(initial?.data ?? null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [offset, setOffset] = useState(initial?.offset ?? 0);
  const resultQuery = useRef(initial?.query ?? "");
  const input = useRef<HTMLInputElement>(null);
  const active = useRef("");
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
    const cached = sessions.get(provider);
    if (cached?.query === q && retry === 0) {
      setData(cached.data);
      setOffset(cached.offset);
      setLoading(false);
      return;
    }
    resultQuery.current = "";
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
            resultQuery.current = q;
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
  useEffect(() => {
    if (!data || resultQuery.current !== query.trim()) return;
    sessions.delete(provider);
    sessions.set(provider, { query: query.trim(), tab, data, offset });
    if (sessions.size > 8) sessions.delete(sessions.keys().next().value!);
  }, [data, query, tab, offset, provider]);
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
      if (active.current !== id) return;
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
      if (active.current === id) setError(String(e));
    } finally {
      if (active.current === id) setLoading(false);
    }
  };
  return (
    <div className="page">
      <header className="page-title">
        <div>
          <h1>Search</h1>
        </div>
      </header>
      <div className="search-box">
        <SearchIcon size={20} />
        <input
          ref={input}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Tracks, artists, playlists or a source link"
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
          detail="Search the selected source’s public catalog."
        />
      ) : tab === "tracks" ? (
        <TrackList
          tracks={data.tracks}
          navigate={navigate}
          listKey={`search:${provider}:${query}`}
        />
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
