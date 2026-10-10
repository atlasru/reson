import { useRef, useState, useEffect, useMemo, useLayoutEffect } from "react";
import { createPortal } from "react-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Heart,
  MoreHorizontal,
  Play,
  Plus,
  SkipForward,
  ExternalLink,
  UserRound,
  ListMusic,
  Trash2,
  FileMusic,
} from "lucide-react";
import { Artwork } from "./Artwork";
import { Empty } from "./States";
import {
  act,
  enqueue,
  favorite,
  original,
  play,
  useLibrary,
  useCurrentTrackId,
  useProviders,
} from "../stores/core";
import type { Route, Track } from "../stores/types";
export const time = (ms: number) =>
  `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`;
const positions = new Map<string, number>();
export function TrackList({
  tracks,
  navigate,
  onRemove,
  removeLabel = "Remove from playlist",
  listKey,
  showProvenance = false,
}: {
  tracks: Track[];
  navigate: (r: Route) => void;
  onRemove?: (index: number) => void;
  removeLabel?: string;
  listKey?: string;
  showProvenance?: boolean;
}) {
  const parent = useRef<HTMLDivElement>(null);
  const menuRoot = useRef<HTMLDivElement>(null);
  const lib = useLibrary();
  const current = useCurrentTrackId();
  const providers = useProviders();
  const liked = useMemo(
    () => new Set(lib.favorites.map((t) => t.internal_id)),
    [lib.favorites],
  );
  const sourceNames = useMemo(
    () => new Map(lib.import_sources.map((s) => [s.id, s.name])),
    [lib.import_sources],
  );
  const [selected, setSelected] = useState(0);
  const [menu, setMenu] = useState<{
    track: Track;
    index: number;
    x: number;
    y: number;
  } | null>(null);
  const virtual = useVirtualizer({
    count: tracks.length,
    getScrollElement: () => parent.current,
    estimateSize: () => 48,
    overscan: 6,
    getItemKey: (i) => `${tracks[i].internal_id}:${i}`,
    initialOffset: listKey ? (positions.get(listKey) ?? 0) : 0,
  });
  useEffect(() => {
    const node = parent.current;
    return () => {
      if (node && listKey) {
        if (positions.size > 64) positions.clear();
        positions.set(listKey, node.scrollTop);
      }
    };
  }, [listKey]);
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    window.addEventListener("click", close);
    window.addEventListener("blur", close);
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        close();
        parent.current?.focus();
      }
    };
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", key);
    };
  }, [menu]);
  useLayoutEffect(() => {
    if (!menu || !menuRoot.current) return;
    const root = menuRoot.current;
    root.style.left = `${Math.max(8, Math.min(menu.x, window.innerWidth - root.offsetWidth - 8))}px`;
    root.style.top = `${Math.max(8, Math.min(menu.y, window.innerHeight - root.offsetHeight - 8))}px`;
    root.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [menu]);
  const openMenu = (track: Track, index: number, e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setSelected(index);
    setMenu({ track, index, x: e.clientX, y: e.clientY });
  };
  const playAt = (index: number) => {
    if (tracks[index]?.availability === "unavailable") return;
    // Queue has a deliberate 5000-entry bound. Play a window containing the selected item.
    const start = Math.max(0, index - 100);
    play(
      tracks.slice(start, start + 5000).map((t) => t.internal_id),
      index - start,
    );
  };
  const keyboard = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.target instanceof HTMLElement && e.target.closest("button")) return;
    let index = Math.min(selected, tracks.length - 1);
    if (e.key === "ArrowDown") index++;
    else if (e.key === "ArrowUp") index--;
    else if (e.key === "Home") index = 0;
    else if (e.key === "End") index = tracks.length - 1;
    else if (e.key === "Enter") {
      e.preventDefault();
      playAt(index);
      return;
    } else if (e.key === "Delete" && onRemove) {
      e.preventDefault();
      onRemove(index);
      return;
    } else if (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) {
      e.preventDefault();
      const rect = parent.current!.getBoundingClientRect();
      setMenu({
        track: tracks[index],
        index,
        x: rect.left + 100,
        y: rect.top + 48,
      });
      return;
    } else return;
    if (e.ctrlKey || e.altKey) return;
    e.preventDefault();
    index = Math.max(0, Math.min(tracks.length - 1, index));
    setSelected(index);
    virtual.scrollToIndex(index, { align: "auto" });
  };
  if (!tracks.length)
    return (
      <Empty
        title="No tracks here yet"
        detail="Search for music or save tracks to your library."
      />
    );
  const action = (fn: () => void) => {
    fn();
    setMenu(null);
    parent.current?.focus();
  };
  return (
    <>
      <div className="track-head">
        <span>#</span>
        <span>Title</span>
        <span>Artist</span>
        <span>Time</span>
        <span />
      </div>
      <div
        className="track-scroll"
        ref={parent}
        role="listbox"
        aria-label="Tracks"
        tabIndex={0}
        onKeyDown={keyboard}
        onScroll={() => setMenu(null)}
      >
        <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
          {virtual.getVirtualItems().map((row) => {
            const t = tracks[row.index];
            const provenance = showProvenance
              ? (lib.import_track_sources[t.internal_id] ?? [])
                  .map((id) => sourceNames.get(id))
                  .filter(Boolean)
                  .join(", ")
              : "";
            const source =
              providers.length > 1
                ? (providers.find((p) => p.id === t.sources[0]?.provider)
                    ?.display_name ?? t.sources[0]?.provider)
                : "";
            const caption = [
              t.availability === "preview"
                ? "Preview"
                : t.availability === "unavailable"
                  ? "Unavailable"
                  : "",
              source,
              provenance ? `From ${provenance}` : "",
            ]
              .filter(Boolean)
              .join(" · ");
            return (
              <div
                key={row.key}
                role="option"
                aria-selected={selected === row.index}
                aria-label={`${t.title}, ${t.artists.map((a) => a.name).join(", ")}`}
                className={`track-row ${selected === row.index ? "selected" : ""} ${current === t.internal_id ? "playing" : ""} ${t.availability === "unavailable" ? "unavailable" : ""}`}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: row.size,
                  transform: `translateY(${row.start}px)`,
                }}
                onClick={() => {
                  setSelected(row.index);
                  parent.current?.focus();
                }}
                onDoubleClick={(e) => {
                  if (
                    !(
                      e.target instanceof HTMLElement &&
                      e.target.closest("button")
                    )
                  )
                    playAt(row.index);
                }}
                onContextMenu={(e) => openMenu(t, row.index, e)}
              >
                <button
                  className="row-number icon"
                  title={`Play ${t.title}`}
                  disabled={t.availability === "unavailable"}
                  onClick={(e) => {
                    e.stopPropagation();
                    playAt(row.index);
                  }}
                >
                  <span>
                    {current === t.internal_id ? (
                      <i className="playing-bars" />
                    ) : (
                      row.index + 1
                    )}
                  </span>
                  <Play size={14} />
                </button>
                <div className="track-name">
                  <Artwork url={t.artwork} thumbnail />
                  <span>
                    <strong title={t.title}>
                      {t.explicit && <b className="explicit">E</b>}
                      {t.title}
                    </strong>
                    {caption && <small title={caption}>{caption}</small>}
                  </span>
                </div>
                <button
                  className="artist-name"
                  title={t.artists.map((a) => a.name).join(", ")}
                  onClick={(e) => {
                    e.stopPropagation();
                    const ref = t.artists[0]?.references[0];
                    if (ref)
                      navigate({
                        page: "artist",
                        provider: ref.provider,
                        id: ref.provider_id,
                      });
                  }}
                >
                  {t.artists.map((a) => a.name).join(", ")}
                </button>
                <span className="duration">{time(t.duration_ms)}</span>
                <div className="row-actions">
                  <button
                    className={`icon ${liked.has(t.internal_id) ? "active" : ""}`}
                    title={
                      liked.has(t.internal_id)
                        ? "Remove from Reson likes"
                        : "Like in Reson"
                    }
                    aria-pressed={liked.has(t.internal_id)}
                    onClick={(e) => {
                      e.stopPropagation();
                      favorite(t.internal_id, !liked.has(t.internal_id));
                    }}
                  >
                    <Heart
                      size={15}
                      fill={liked.has(t.internal_id) ? "currentColor" : "none"}
                    />
                  </button>
                  <button
                    className="icon"
                    title="Track actions"
                    onClick={(e) => openMenu(t, row.index, e)}
                  >
                    <MoreHorizontal size={17} />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </div>
      {menu &&
        createPortal(
          <div
            className="context-menu"
            role="menu"
            aria-label="Track actions"
            ref={menuRoot}
            style={{ left: menu.x, top: menu.y }}
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (
                ["ArrowDown", "ArrowUp", "Home", "End", "Tab"].includes(e.key)
              ) {
                e.preventDefault();
                const items = Array.from(
                  menuRoot.current!.querySelectorAll<HTMLButtonElement>(
                    "button:not(:disabled)",
                  ),
                );
                const i = items.indexOf(
                  document.activeElement as HTMLButtonElement,
                );
                items[
                  e.key === "Home"
                    ? 0
                    : e.key === "End"
                      ? items.length - 1
                      : (i +
                          (e.key === "ArrowUp" ||
                          (e.key === "Tab" && e.shiftKey)
                            ? -1
                            : 1) +
                          items.length) %
                        items.length
                ]?.focus();
              }
            }}
          >
            <button
              role="menuitem"
              disabled={menu.track.availability === "unavailable"}
              onClick={() => action(() => playAt(menu.index))}
            >
              <Play size={15} />
              Play now
            </button>
            <button
              role="menuitem"
              onClick={() =>
                action(() => enqueue([menu.track.internal_id], true))
              }
            >
              <SkipForward size={15} />
              Play next
            </button>
            <button
              role="menuitem"
              onClick={() => action(() => enqueue([menu.track.internal_id]))}
            >
              <Plus size={15} />
              Add to queue
            </button>
            <button
              role="menuitem"
              onClick={() =>
                action(() =>
                  favorite(
                    menu.track.internal_id,
                    !liked.has(menu.track.internal_id),
                  ),
                )
              }
            >
              <Heart size={15} />
              {liked.has(menu.track.internal_id)
                ? "Remove from Reson likes"
                : "Like in Reson"}
            </button>
            <div className="menu-divider" />
            <button
              role="menuitem"
              onClick={() =>
                action(() => navigate({ page: "track", track: menu.track }))
              }
            >
              <FileMusic size={15} />
              Open track
            </button>
            <button
              role="menuitem"
              onClick={() =>
                action(() => {
                  const ref = menu.track.artists[0]?.references[0];
                  if (ref)
                    navigate({
                      page: "artist",
                      provider: ref.provider,
                      id: ref.provider_id,
                    });
                })
              }
            >
              <UserRound size={15} />
              Open artist
            </button>
            {menu.track.sources[0]?.url && (
              <button
                role="menuitem"
                onClick={() =>
                  action(() => original(menu.track.sources[0].url!))
                }
              >
                <ExternalLink size={15} />
                Open on{" "}
                {providers.find((p) => p.id === menu.track.sources[0].provider)
                  ?.display_name ?? menu.track.sources[0].provider}
              </button>
            )}
            {lib.playlists.length > 0 && (
              <>
                <div className="menu-label">Add to playlist</div>
                <div className="menu-playlists">
                  {lib.playlists.map((p) => (
                    <button
                      role="menuitem"
                      key={p.internal_id}
                      onClick={() =>
                        action(() =>
                          act("add_to_playlist", {
                            id: p.internal_id,
                            ids: [menu.track.internal_id],
                          }),
                        )
                      }
                    >
                      <ListMusic size={14} />
                      {p.title}
                    </button>
                  ))}
                </div>
              </>
            )}
            {onRemove && (
              <>
                <div className="menu-divider" />
                <button
                  role="menuitem"
                  onClick={() => action(() => onRemove(menu.index))}
                >
                  <Trash2 size={15} />
                  {removeLabel}
                </button>
              </>
            )}
          </div>,
          document.body,
        )}
    </>
  );
}
