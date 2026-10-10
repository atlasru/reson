import { useMemo, useState } from "react";
import { Download, Plus, Play, Pencil, Trash2, RefreshCw } from "lucide-react";
import {
  act,
  call,
  importActive,
  play,
  rememberImport,
  useImports,
  useLibrary,
} from "../stores/core";
import type { ImportProgress, ImportSource, Route } from "../stores/types";
import { TrackList } from "../components/TrackList";
import { Artwork } from "../components/Artwork";
import { Empty } from "../components/States";
import { Dialog } from "../components/Dialog";
import { ImportDialog } from "../library/ImportDialog";

export function LibraryView({
  navigate,
  mode = "library",
}: {
  navigate: (r: Route) => void;
  mode?: "library" | "liked" | "playlists";
}) {
  const lib = useLibrary();
  const jobs = useImports();
  const [tab, setTab] = useState<"liked" | "recent" | "playlists">(
    mode === "playlists" ? "playlists" : mode === "liked" ? "liked" : "recent",
  );
  const [filter, setFilter] = useState("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("added");
  const [editor, setEditor] = useState<{ id?: string; title: string } | null>(
    null,
  );
  const [importDialog, setImportDialog] = useState<{
    input?: string;
    job?: string;
  } | null>(null);
  const [sourcesOpen, setSourcesOpen] = useState(false);
  const [removing, setRemoving] = useState<ImportSource | null>(null);
  const [removeTracks, setRemoveTracks] = useState(false);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const localLikes = useMemo(
    () => new Set(lib.local_favorite_ids),
    [lib.local_favorite_ids],
  );
  const tracks = useMemo(() => {
    let rows = tab === "liked" ? lib.favorites : lib.recent;
    if (tab === "liked" && filter !== "all")
      rows = rows.filter((t) =>
        filter === "local"
          ? localLikes.has(t.internal_id)
          : lib.import_track_sources[t.internal_id]?.includes(filter),
      );
    const q = query.toLocaleLowerCase().trim();
    if (q)
      rows = rows.filter((t) =>
        `${t.title} ${t.artists.map((a) => a.name).join(" ")}`
          .toLocaleLowerCase()
          .includes(q),
      );
    if (sort !== "added")
      rows = [...rows].sort((a, b) =>
        sort === "title"
          ? a.title.localeCompare(b.title)
          : (a.artists[0]?.name.localeCompare(b.artists[0]?.name ?? "") ?? 0),
      );
    return rows;
  }, [lib, localLikes, tab, filter, query, sort]);
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
  const refresh = async (source: ImportSource) => {
    setError("");
    const running = jobs.find(
      (p) => p.source_id === source.id && importActive(p),
    );
    if (running) {
      setImportDialog({ input: source.url, job: running.job_id });
      return;
    }
    try {
      const p = await call<ImportProgress>("start_likes_import", {
        provider: source.provider,
        input: source.url,
      });
      rememberImport(p);
      setImportDialog({ input: source.url, job: p.job_id });
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="page library-page">
      <header className="page-title">
        <div>
          <h1>
            {mode === "liked"
              ? "Liked Tracks"
              : mode === "playlists"
                ? "Playlists"
                : "Library"}
          </h1>
          <p className="page-caption">
            {tab === "liked"
              ? `${lib.favorites.length} tracks saved in Reson`
              : tab === "recent"
                ? "Recently played"
                : `${lib.playlists.length} local playlists`}
          </p>
        </div>
        <div className="button-row">
          {tab === "liked" && (
            <button
              className="secondary"
              data-action="import"
              onClick={() => setImportDialog({})}
            >
              <Download size={15} />
              Import from SoundCloud
            </button>
          )}
          {tab === "playlists" && (
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
          )}
          {tab !== "playlists" && tracks.length > 0 && (
            <button
              className="primary"
              onClick={() =>
                play(tracks.slice(0, 5000).map((t) => t.internal_id))
              }
            >
              <Play size={14} fill="currentColor" />
              Play
            </button>
          )}
        </div>
      </header>
      {mode === "library" && (
        <div className="tabs">
          <button
            className={tab === "recent" ? "selected" : ""}
            onClick={() => setTab("recent")}
          >
            Recently played
          </button>
          <button
            className={tab === "liked" ? "selected" : ""}
            onClick={() => setTab("liked")}
          >
            Liked Tracks
          </button>
          <button
            className={tab === "playlists" ? "selected" : ""}
            onClick={() => setTab("playlists")}
          >
            Playlists
          </button>
        </div>
      )}
      {tab === "liked" && (
        <>
          <div className="library-toolbar">
            <input
              aria-label="Filter library"
              placeholder="Filter tracks"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <select
              aria-label="Import source"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            >
              <option value="all">All saved tracks</option>
              <option value="local">My local likes</option>
              {lib.import_sources.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
            <select
              aria-label="Sort tracks"
              value={sort}
              onChange={(e) => setSort(e.target.value)}
            >
              <option value="added">Recently added</option>
              <option value="title">Title</option>
              <option value="artist">Artist</option>
            </select>
            {lib.import_sources.length > 0 && (
              <button
                className="text-button"
                aria-expanded={sourcesOpen}
                onClick={() => setSourcesOpen((v) => !v)}
              >
                Sources ({lib.import_sources.length})
              </button>
            )}
          </div>
          {sourcesOpen && (
            <div className="import-sources">
              {lib.import_sources.map((s) => {
                const p =
                  jobs.filter((p) => p.source_id === s.id).at(-1) ?? s.progress;
                return (
                  <div className="import-source" key={s.id}>
                    <Artwork url={s.artwork} thumbnail />
                    <div>
                      <strong>{s.name}</strong>
                      <small>
                        {s.track_count} tracks
                        {p
                          ? ` · ${p.status === "complete" ? "Up to date" : p.status === "cooldown" ? "Waiting for SoundCloud" : p.status === "importing" ? "Importing" : p.status === "partial" ? "Partial import" : p.status === "cancelled" ? "Cancelled" : p.status === "interrupted" ? "Interrupted" : p.status === "failed" ? "Failed" : "Resolving"}`
                          : ""}
                      </small>
                      {p?.message && (
                        <small
                          className={
                            p.status === "partial" || p.status === "failed"
                              ? "inline-error"
                              : ""
                          }
                        >
                          {p.message}
                        </small>
                      )}
                    </div>
                    <button
                      className="icon"
                      title={`Refresh ${s.name}`}
                      onClick={() => void refresh(s)}
                    >
                      <RefreshCw size={15} />
                    </button>
                    <button
                      className="icon"
                      title={`Remove source ${s.name}`}
                      onClick={() => {
                        setRemoving(s);
                        setRemoveTracks(false);
                        setError("");
                      }}
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}
          {jobs.filter(importActive).map((p) => (
            <button
              key={p.job_id}
              className="import-banner"
              onClick={() => setImportDialog({ job: p.job_id })}
            >
              <span className="activity-dot" />
              <span>
                {p.profile} · {p.saved} saved
                {p.status === "cooldown"
                  ? " · Waiting for rate limit"
                  : " · Importing"}
              </span>
              <span>View progress</span>
            </button>
          ))}
        </>
      )}
      {error && !editor && !removing && (
        <p className="inline-error" role="alert">
          {error}
        </p>
      )}
      {tab !== "playlists" ? (
        tracks.length > 0 ? (
          <TrackList
            key={`${tab}:${filter}:${sort}`}
            tracks={tracks}
            navigate={navigate}
            listKey={`library:${tab}:${filter}:${sort}`}
            showProvenance={tab === "liked"}
            onRemove={
              tab === "liked"
                ? (index) =>
                    act("remove_saved_track", { id: tracks[index].internal_id })
                : undefined
            }
            removeLabel="Remove from library"
          />
        ) : (
          <Empty
            title={
              query
                ? "No matching tracks"
                : tab === "liked"
                  ? "Save music you want to keep"
                  : "Nothing played yet"
            }
            detail={
              tab === "liked"
                ? "Like a track in Reson, or import a public SoundCloud profile’s likes."
                : "Find music to start listening."
            }
            action={
              tab === "liked" ? (
                <button
                  className="secondary"
                  onClick={() => setImportDialog({})}
                >
                  Import from SoundCloud
                </button>
              ) : (
                <button
                  className="secondary"
                  onClick={() => navigate({ page: "search" })}
                >
                  Search music
                </button>
              )
            }
          />
        )
      ) : lib.playlists.length === 0 ? (
        <Empty
          title="Your playlists"
          detail="Create a playlist, then add tracks from their menus."
          action={
            <button
              className="secondary"
              onClick={() => setEditor({ title: "" })}
            >
              New playlist
            </button>
          }
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
                  <small>{p.track_count} tracks</small>
                </span>
              </button>
              <button
                className="icon"
                title="Rename playlist"
                onClick={() => setEditor({ id: p.internal_id, title: p.title })}
              >
                <Pencil size={15} />
              </button>
              <button
                className="icon danger"
                title="Delete local playlist"
                onClick={() => act("delete_playlist", { id: p.internal_id })}
              >
                <Trash2 size={15} />
              </button>
            </div>
          ))}
        </div>
      )}
      {tab === "liked" && (
        <p className="library-note">
          Local likes and imported snapshots. Your SoundCloud account is
          unchanged.
        </p>
      )}
      {editor && (
        <Dialog
          title={editor.id ? "Rename playlist" : "New playlist"}
          close={() => setEditor(null)}
        >
          <form onSubmit={(e) => void submit(e)}>
            <label className="field-label" htmlFor="playlist-name">
              Name
            </label>
            <input
              id="playlist-name"
              value={editor.title}
              maxLength={120}
              onChange={(e) => setEditor({ ...editor, title: e.target.value })}
            />
            {error && <p className="inline-error">{error}</p>}
            <footer className="dialog-actions">
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
            </footer>
          </form>
        </Dialog>
      )}
      {importDialog && (
        <ImportDialog
          initialInput={importDialog.input}
          initialJob={importDialog.job}
          close={() => setImportDialog(null)}
        />
      )}
      {removing && (
        <Dialog title="Remove imported source" close={() => setRemoving(null)}>
          <p className="dialog-description">
            Remove {removing.name} from your import sources. Tracks stay in your
            local likes unless you choose below.
          </p>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={removeTracks}
              onChange={(e) => setRemoveTracks(e.target.checked)}
            />
            Also remove tracks only saved by this source
          </label>
          {error && <p className="inline-error">{error}</p>}
          <footer className="dialog-actions">
            <button className="secondary" onClick={() => setRemoving(null)}>
              Cancel
            </button>
            <button
              className="primary"
              onClick={() => {
                void call("remove_import_source", {
                  id: removing.id,
                  removeTracks,
                })
                  .then(() => {
                    if (filter === removing.id) setFilter("all");
                    setRemoving(null);
                  })
                  .catch((e) => setError(String(e)));
              }}
            >
              Remove source
            </button>
          </footer>
        </Dialog>
      )}
    </div>
  );
}
