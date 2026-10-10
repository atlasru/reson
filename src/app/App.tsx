import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Heart,
  Home as HomeIcon,
  Library as LibraryIcon,
  ListMusic,
  PanelLeftClose,
  PanelLeftOpen,
  Search as SearchIcon,
  Settings as SettingsIcon,
  WifiOff,
  X,
} from "lucide-react";
import {
  initialize,
  control,
  player,
  useNotice,
  notices,
  useLibrary,
  useImports,
  importActive,
} from "../stores/core";
import type { Route } from "../stores/types";
import { Home } from "../pages/Home";
import { Search } from "../pages/Search";
import { LibraryView } from "../pages/Library";
import { ArtistView } from "../pages/Artist";
import { PlaylistView } from "../pages/Playlist";
import { TrackView } from "../pages/Track";
import { SettingsView } from "../pages/Settings";
import { PlayerBar } from "../player/PlayerBar";
import { QueuePanel } from "../player/QueuePanel";
import { Artwork } from "../components/Artwork";
import { ImportDialog } from "../library/ImportDialog";

export function App() {
  const [routes, setRoutes] = useState<Route[]>([{ page: "home" }]);
  const [cursor, setCursor] = useState(0);
  const history = useRef({ routes, cursor });
  history.current = { routes, cursor };
  const route = routes[cursor];
  const [queueOpen, setQueueOpen] = useState(false);
  const [ready, setReady] = useState(false);
  const [fatal, setFatal] = useState("");
  const [collapsed, setCollapsed] = useState(
    localStorage.getItem("reson.sidebar.collapsed") === "true",
  );
  const [sidebarWidth, setSidebarWidth] = useState(
    Math.max(
      172,
      Math.min(280, Number(localStorage.getItem("reson.sidebar.width")) || 200),
    ),
  );
  const [importJob, setImportJob] = useState<string>();
  const notice = useNotice();
  const lib = useLibrary();
  const jobs = useImports();
  const activeImports = jobs.filter(importActive);
  const [online, setOnline] = useState(navigator.onLine);
  const navigate = useCallback((r: Route) => {
    const current = history.current;
    const currentRoute = current.routes[current.cursor];
    if (
      r.page === currentRoute.page &&
      !["track", "artist", "playlist"].includes(r.page)
    )
      return;
    setRoutes([...current.routes.slice(0, current.cursor + 1), r]);
    setCursor(current.cursor + 1);
  }, []);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void initialize()
      .then((fn) => {
        if (disposed) fn();
        else {
          unlisten = fn;
          setReady(true);
        }
      })
      .catch((e) => setFatal(String(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  useEffect(() => {
    localStorage.setItem("reson.sidebar.collapsed", String(collapsed));
    localStorage.setItem("reson.sidebar.width", String(sidebarWidth));
  }, [collapsed, sidebarWidth]);
  useEffect(() => {
    const update = () => setOnline(navigator.onLine);
    window.addEventListener("online", update);
    window.addEventListener("offline", update);
    return () => {
      window.removeEventListener("online", update);
      window.removeEventListener("offline", update);
    };
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => {
      if (notices.get()?.id === notice.id) notices.set(null);
    }, 8000);
    return () => clearTimeout(timer);
  }, [notice]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (document.querySelector('[role="dialog"]')) return;
      const input =
        e.target instanceof HTMLElement &&
        e.target.closest('input,textarea,select,[contenteditable="true"]');
      if (e.ctrlKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (route.page !== "search") navigate({ page: "search" });
        else window.dispatchEvent(new Event("focus-search"));
        return;
      }
      if (input) return;
      if (
        e.code === "Space" &&
        !(
          e.target instanceof HTMLElement &&
          e.target.closest("button,[role=menu]")
        )
      ) {
        e.preventDefault();
        if (!e.repeat) control("toggle");
      } else if (e.altKey && e.key === "ArrowRight") {
        e.preventDefault();
        control("next");
      } else if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault();
        control("previous");
      } else if (e.ctrlKey && e.key.toLowerCase() === "q") {
        e.preventDefault();
        setQueueOpen((v) => !v);
      } else if (e.key === "Escape") setQueueOpen(false);
      else if (e.ctrlKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
        e.preventDefault();
        control(
          "volume",
          Math.max(
            0,
            Math.min(
              1,
              player.get().volume + (e.key === "ArrowUp" ? 0.05 : -0.05),
            ),
          ),
        );
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [route.page, navigate]);
  const nav = (
    items: {
      page: "home" | "search" | "library" | "liked" | "playlists";
      label: string;
      icon: typeof HomeIcon;
    }[],
  ) =>
    items.map((item) => (
      <button
        key={item.page}
        data-nav={item.page}
        title={collapsed ? item.label : undefined}
        aria-label={item.label}
        aria-current={route.page === item.page ? "page" : undefined}
        className={route.page === item.page ? "selected" : ""}
        onClick={() => navigate({ page: item.page })}
      >
        <item.icon size={18} />
        <span>{item.label}</span>
        {item.page === "liked" && !collapsed && lib.favorites.length > 0 && (
          <small>{lib.favorites.length}</small>
        )}
      </button>
    ));
  return (
    <div
      className={`app-shell ${collapsed ? "sidebar-collapsed" : ""}`}
      style={
        {
          "--sidebar-width": `${collapsed ? 64 : sidebarWidth}px`,
        } as React.CSSProperties
      }
    >
      <aside className="sidebar">
        <div className="brand">
          <svg width="24" height="28" viewBox="0 0 30 32" aria-hidden="true">
            <path
              d="M4 10v12M11 4v24M18 8v16M25 13v6"
              stroke="currentColor"
              strokeWidth="3"
              strokeLinecap="round"
            />
          </svg>
          <span>Reson</span>
        </div>
        <nav aria-label="Browse">
          {nav([
            { page: "home", label: "Home", icon: HomeIcon },
            { page: "search", label: "Search", icon: SearchIcon },
          ])}
        </nav>
        <div className="nav-divider" />
        <nav aria-label="Your library">
          {nav([
            { page: "library", label: "Library", icon: LibraryIcon },
            { page: "liked", label: "Liked Tracks", icon: Heart },
            { page: "playlists", label: "Playlists", icon: ListMusic },
          ])}
        </nav>
        {!collapsed && (
          <div className="sidebar-playlists">
            {lib.playlists.map((p) => (
              <button
                key={p.internal_id}
                className={
                  route.page === "playlist" &&
                  route.playlist.internal_id === p.internal_id
                    ? "selected"
                    : ""
                }
                title={p.title}
                onClick={() => navigate({ page: "playlist", playlist: p })}
              >
                <Artwork url={p.artwork} />
                <span>{p.title}</span>
              </button>
            ))}
          </div>
        )}
        <div className="sidebar-bottom">
          {activeImports.map((p) => (
            <button
              className="sidebar-import"
              key={p.job_id}
              title={`Import: ${p.profile}`}
              onClick={() => setImportJob(p.job_id)}
            >
              <span className="activity-dot" />
              <span>
                {p.status === "cooldown" ? "Import waiting" : "Importing likes"}
              </span>
            </button>
          ))}
          <button
            data-nav="settings"
            aria-label="Settings"
            aria-current={route.page === "settings" ? "page" : undefined}
            title={collapsed ? "Settings" : undefined}
            className={route.page === "settings" ? "selected" : ""}
            onClick={() => navigate({ page: "settings" })}
          >
            <SettingsIcon size={18} />
            <span>Settings</span>
          </button>
        </div>
        {!collapsed && (
          <div
            className="sidebar-resize"
            role="separator"
            aria-label="Resize sidebar"
            aria-orientation="vertical"
            tabIndex={0}
            onKeyDown={(e) => {
              if (["ArrowLeft", "ArrowRight"].includes(e.key)) {
                e.preventDefault();
                setSidebarWidth((w) =>
                  Math.max(
                    172,
                    Math.min(280, w + (e.key === "ArrowRight" ? 8 : -8)),
                  ),
                );
              }
            }}
            onPointerDown={(e) =>
              e.currentTarget.setPointerCapture(e.pointerId)
            }
            onPointerMove={(e) => {
              if (e.currentTarget.hasPointerCapture(e.pointerId))
                setSidebarWidth(Math.max(172, Math.min(280, e.clientX)));
            }}
          />
        )}
      </aside>
      <main className="main-area">
        <div className="topbar">
          <button
            className="icon"
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            onClick={() => setCollapsed((v) => !v)}
          >
            {collapsed ? (
              <PanelLeftOpen size={17} />
            ) : (
              <PanelLeftClose size={17} />
            )}
          </button>
          <div className="history-buttons">
            <button
              className="icon"
              title="Back"
              disabled={cursor === 0}
              onClick={() => setCursor((c) => c - 1)}
            >
              <ArrowLeft size={17} />
            </button>
            <button
              className="icon"
              title="Forward"
              disabled={cursor === routes.length - 1}
              onClick={() => setCursor((c) => c + 1)}
            >
              <ArrowRight size={17} />
            </button>
          </div>
          {!online && (
            <span className="offline-status">
              <WifiOff size={14} />
              Offline
            </span>
          )}
          <button
            className="topbar-search"
            onClick={() => {
              navigate({ page: "search" });
              window.dispatchEvent(new Event("focus-search"));
            }}
          >
            <SearchIcon size={15} />
            <span>Search music</span>
            <kbd>Ctrl K</kbd>
          </button>
        </div>
        <div className="content-area">
          {fatal ? (
            <div className="startup-error">
              <h2>Reson couldn’t start</h2>
              <p>{fatal}</p>
              <button className="secondary" onClick={() => location.reload()}>
                Retry
              </button>
            </div>
          ) : !ready ? (
            <div className="startup">Starting Reson…</div>
          ) : route.page === "home" ? (
            <Home navigate={navigate} />
          ) : route.page === "search" ? (
            <Search navigate={navigate} />
          ) : route.page === "library" ||
            route.page === "liked" ||
            route.page === "playlists" ? (
            <LibraryView
              key={route.page}
              navigate={navigate}
              mode={route.page}
            />
          ) : route.page === "artist" ? (
            <ArtistView
              key={route.provider + route.id}
              provider={route.provider}
              id={route.id}
              navigate={navigate}
            />
          ) : route.page === "playlist" ? (
            <PlaylistView
              key={route.playlist.internal_id}
              playlist={route.playlist}
              navigate={navigate}
            />
          ) : route.page === "track" ? (
            <TrackView
              key={route.track.internal_id}
              track={route.track}
              navigate={navigate}
            />
          ) : (
            <SettingsView />
          )}
        </div>
      </main>
      {queueOpen && <QueuePanel close={() => setQueueOpen(false)} />}
      <PlayerBar
        queueOpen={queueOpen}
        onQueue={() => setQueueOpen((v) => !v)}
        navigate={navigate}
      />
      {importJob && (
        <ImportDialog
          initialJob={importJob}
          close={() => setImportJob(undefined)}
        />
      )}
      {notice && (
        <div className="toast" role="status">
          <span>{notice.text}</span>
          <button
            className="icon"
            title="Dismiss"
            onClick={() => notices.set(null)}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
