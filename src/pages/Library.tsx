import { useState } from "react";
import { Plus, Play, Pencil, Trash2, X } from "lucide-react";
import { act, call, play, useLibrary } from "../stores/core";
import type { Route } from "../stores/types";
import { TrackList } from "../components/TrackList";
import { Artwork } from "../components/Artwork";
import { Empty } from "../components/States";
export function LibraryView({
  navigate,
  playlistsOnly = false,
}: {
  navigate: (r: Route) => void;
  playlistsOnly?: boolean;
}) {
  const lib = useLibrary();
  const [tab, setTab] = useState<"favorites" | "recent" | "playlists">(
    playlistsOnly ? "playlists" : "favorites",
  );
  const [editor, setEditor] = useState<{ id?: string; title: string } | null>(
    null,
  );
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const tracks = tab === "favorites" ? lib.favorites : lib.recent;
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editor) return;
    setSaving(true);
    setError("");
    try {
      await call(
        editor.id ? "rename_playlist" : "create_playlist",
        editor.id
          ? { id: editor.id, title: editor.title }
          : { title: editor.title },
      );
      setEditor(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="page">
      <header className="page-title">
        <div>
          <span className="eyebrow">Stored on this device</span>
          <h1>{playlistsOnly ? "Playlists" : "Your library"}</h1>
        </div>
        <div className="button-row">
          {tab !== "playlists" && tracks.length > 0 && (
            <button
              className="secondary"
              onClick={() => play(tracks.map((t) => t.internal_id))}
            >
              <Play size={15} />
              Play all
            </button>
          )}
          <button
            className="secondary"
            onClick={() => {
              setEditor({ title: "" });
              setError("");
            }}
          >
            <Plus size={15} />
            New playlist
          </button>
        </div>
      </header>
      {!playlistsOnly && (
        <div className="tabs">
          <button
            className={tab === "favorites" ? "selected" : ""}
            onClick={() => setTab("favorites")}
          >
            Favorites <span>{lib.favorites.length}</span>
          </button>
          <button
            className={tab === "recent" ? "selected" : ""}
            onClick={() => setTab("recent")}
          >
            Recently played
          </button>
          <button
            className={tab === "playlists" ? "selected" : ""}
            onClick={() => setTab("playlists")}
          >
            Playlists
          </button>
        </div>
      )}
      {tab !== "playlists" ? (
        <TrackList tracks={tracks} navigate={navigate} />
      ) : lib.playlists.length === 0 ? (
        <Empty
          title="Make a playlist your own"
          detail="Create a local playlist, then add music from any track’s menu."
        />
      ) : (
        <div className="page-scroll local-playlists">
          {lib.playlists.map((p) => (
            <div className="local-playlist" key={p.internal_id}>
              <button
                className="playlist-open"
                onClick={() => navigate({ page: "playlist", playlist: p })}
              >
                <Artwork url={p.artwork} />
                <span>
                  <strong>{p.title}</strong>
                  <small>{p.track_count} tracks · Reson</small>
                </span>
              </button>
              <button
                className="icon"
                title="Rename playlist"
                onClick={() => setEditor({ id: p.internal_id, title: p.title })}
              >
                <Pencil size={16} />
              </button>
              <button
                className="icon danger"
                title="Delete local playlist"
                onClick={() => act("delete_playlist", { id: p.internal_id })}
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
        </div>
      )}
      {editor && (
        <div
          className="modal-backdrop"
          onMouseDown={(e) => {
            if (e.target === e.currentTarget) setEditor(null);
          }}
        >
          <form className="dialog" onSubmit={(e) => void submit(e)}>
            <div className="section-title">
              <h2>{editor.id ? "Rename playlist" : "New playlist"}</h2>
              <button
                className="icon"
                type="button"
                title="Close"
                onClick={() => setEditor(null)}
              >
                <X size={18} />
              </button>
            </div>
            <label>
              Name
              <input
                autoFocus
                value={editor.title}
                maxLength={120}
                onChange={(e) =>
                  setEditor({ ...editor, title: e.target.value })
                }
              />
            </label>
            {error && <p className="inline-error">{error}</p>}
            <div className="button-row">
              <button
                className="secondary"
                type="button"
                onClick={() => setEditor(null)}
              >
                Cancel
              </button>
              <button
                className="primary"
                disabled={saving || !editor.title.trim()}
              >
                {saving ? "Saving…" : "Save playlist"}
              </button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}
