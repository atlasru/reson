import { useEffect, useState } from "react";
import { Play, Plus, ExternalLink, ArrowDown } from "lucide-react";
import { call, enqueue, original, play } from "../stores/core";
import type { ArtistPage, Route } from "../stores/types";
import { Artwork } from "../components/Artwork";
import { TrackList } from "../components/TrackList";
import { PlaylistCards } from "../components/Cards";
import { Failure, Skeleton } from "../components/States";
export function ArtistView({
  provider,
  id,
  navigate,
}: {
  provider: string;
  id: string;
  navigate: (r: Route) => void;
}) {
  const [data, setData] = useState<ArtistPage | null>(null);
  const [tab, setTab] = useState<"tracks" | "playlists">("tracks");
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const [loading, setLoading] = useState(true);
  const [offset, setOffset] = useState(0);
  useEffect(() => {
    let alive = true;
    setLoading(true);
    setError("");
    setOffset(0);
    void call<ArtistPage>("artist_page", { provider, id, offset: 0 })
      .then((r) => {
        if (alive) setData(r);
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
  }, [provider, id, retry]);
  const more = async () => {
    setLoading(true);
    try {
      const next = offset + 50;
      const r = await call<ArtistPage>("artist_page", {
        provider,
        id,
        offset: next,
      });
      setData((old) =>
        old
          ? {
              ...old,
              tracks: [...old.tracks, ...r.tracks],
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
  if (loading && !data)
    return (
      <div className="page">
        <Skeleton />
      </div>
    );
  if (!data)
    return (
      <div className="page">
        <Failure message={error} retry={() => setRetry((n) => n + 1)} />
      </div>
    );
  return (
    <div className="page">
      <header className="entity-header">
        <Artwork url={data.artist.artwork} className="artist-hero" />
        <div>
          <span className="eyebrow">Artist / user · {provider}</span>
          <h1>{data.artist.name}</h1>
          <p className="description">{data.artist.description}</p>
          <div className="button-row">
            <button
              className="primary"
              onClick={() => play(data.tracks.map((t) => t.internal_id))}
              disabled={!data.tracks.length}
            >
              <Play size={16} fill="currentColor" />
              Play tracks
            </button>
            <button
              className="secondary"
              onClick={() => enqueue(data.tracks.map((t) => t.internal_id))}
              disabled={!data.tracks.length}
            >
              <Plus size={16} />
              Queue
            </button>
            {data.artist.references[0]?.url && (
              <button
                className="icon"
                title="Open artist on source"
                onClick={() => original(data.artist.references[0].url!)}
              >
                <ExternalLink size={17} />
              </button>
            )}
          </div>
        </div>
      </header>
      <div className="tabs">
        <button
          className={tab === "tracks" ? "selected" : ""}
          onClick={() => setTab("tracks")}
        >
          Tracks
        </button>
        <button
          className={tab === "playlists" ? "selected" : ""}
          onClick={() => setTab("playlists")}
        >
          Playlists
        </button>
      </div>
      {tab === "tracks" ? (
        <TrackList tracks={data.tracks} navigate={navigate} />
      ) : (
        <div className="page-scroll">
          <PlaylistCards playlists={data.playlists} navigate={navigate} />
        </div>
      )}
      {data.has_more && (
        <button
          className="secondary load-more"
          disabled={loading}
          onClick={() => void more()}
        >
          <ArrowDown size={14} />
          {loading ? "Loading…" : "Load more"}
        </button>
      )}
      {error && <p className="inline-error">{error}</p>}
    </div>
  );
}
