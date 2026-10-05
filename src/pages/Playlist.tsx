import { useEffect, useState } from "react";
import { Play, Plus, Shuffle, ExternalLink } from "lucide-react";
import {
  act,
  call,
  control,
  enqueue,
  original,
  play,
  useLibrary,
} from "../stores/core";
import type { Playlist, Route } from "../stores/types";
import { Artwork } from "../components/Artwork";
import { TrackList } from "../components/TrackList";
import { Failure, Skeleton } from "../components/States";
export function PlaylistView({
  playlist,
  navigate,
}: {
  playlist: Playlist;
  navigate: (r: Route) => void;
}) {
  const lib = useLibrary();
  const [data, setData] = useState(playlist);
  const [loading, setLoading] = useState(!!playlist.reference);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  const reference = playlist.reference;
  const provider = reference?.provider;
  const id = reference?.provider_id;
  useEffect(() => {
    if (!provider || !id) return;
    let alive = true;
    setLoading(true);
    setError("");
    void call<Playlist>("provider_playlist", { provider, id })
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
  const p = reference
    ? data
    : (lib.playlists.find((p) => p.internal_id === playlist.internal_id) ??
      data);
  if (loading)
    return (
      <div className="page">
        <Skeleton />
      </div>
    );
  if (error)
    return (
      <div className="page">
        <Failure message={error} retry={() => setRetry((n) => n + 1)} />
      </div>
    );
  return (
    <div className="page">
      <header className="entity-header">
        <Artwork url={p.artwork} />
        <div>
          <span className="eyebrow">
            {reference ? `${provider} playlist` : "Local Reson playlist"}
          </span>
          <h1>{p.title}</h1>
          <p className="description">
            {p.owner?.name ? `${p.owner.name} · ` : ""}
            {p.tracks.length} tracks
            {p.track_count > p.tracks.length
              ? ` · ${p.track_count - p.tracks.length} unavailable`
              : ""}
            {p.description ? ` · ${p.description}` : ""}
          </p>
          <div className="button-row">
            <button
              className="primary"
              disabled={!p.tracks.length}
              onClick={() => play(p.tracks.map((t) => t.internal_id))}
            >
              <Play size={16} fill="currentColor" />
              Play
            </button>
            <button
              className="secondary"
              disabled={!p.tracks.length}
              onClick={() => {
                play(
                  p.tracks.map((t) => t.internal_id),
                  Math.floor(Math.random() * p.tracks.length),
                );
                control("shuffle", true);
              }}
            >
              <Shuffle size={16} />
              Shuffle
            </button>
            <button
              className="secondary"
              disabled={!p.tracks.length}
              onClick={() => enqueue(p.tracks.map((t) => t.internal_id))}
            >
              <Plus size={16} />
              Queue
            </button>
            {p.reference?.url && (
              <button
                className="icon"
                title="Open playlist on source"
                onClick={() => original(p.reference!.url!)}
              >
                <ExternalLink size={17} />
              </button>
            )}
          </div>
        </div>
      </header>
      <TrackList
        tracks={p.tracks}
        navigate={navigate}
        onRemove={
          reference
            ? undefined
            : (index) =>
                act("remove_from_playlist", {
                  id: p.internal_id,
                  position: index,
                })
        }
      />
    </div>
  );
}
