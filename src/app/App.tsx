import { useCallback, useEffect, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  Cloud,
  Home as HomeIcon,
  Library as LibraryIcon,
  ListMusic,
  Search as SearchIcon,
  Settings as SettingsIcon,
  X,
  WifiOff,
} from "lucide-react";
import {
  initialize,
  control,
  player,
  useNotice,
  useProviders,
  selectedProvider,
  useSelectedProvider,
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
export function App() {
  const [routes, setRoutes] = useState<Route[]>([{ page: "home" }]);
  const [cursor, setCursor] = useState(0);
  const route = routes[cursor];
  const [queueOpen, setQueueOpen] = useState(false);
  const [ready, setReady] = useState(false);
  const [fatal, setFatal] = useState("");
  const notice = useNotice();
  const [online, setOnline] = useState(navigator.onLine);
  const sources = useProviders();
  const selectedSource = useSelectedProvider();
  const navigate = useCallback(
    (r: Route) => {
      setRoutes((old) => [...old.slice(0, cursor + 1), r]);
      setCursor((old) => old + 1);
    },
    [cursor],
  );
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
    const timer = setTimeout(() => notifyClear(notice.id), 6000);
    return () => clearTimeout(timer);
  }, [notice]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const input =
        e.target instanceof HTMLElement &&
        e.target.closest('input,textarea,select,[contenteditable="true"]') !==
          null;
      if (e.ctrlKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (route.page !== "search") navigate({ page: "search" });
        else window.dispatchEvent(new Event("focus-search"));
        return;
      }
      if (input) return;
      if (e.code === "Space") {
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
      } else if (e.ctrlKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
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
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <svg width="27" height="30" viewBox="0 0 30 32" aria-hidden="true">
            <path
              d="M4 10v12M11 4v24M18 8v16M25 13v6"
              stroke="currentColor"
              strokeWidth="3.2"
              strokeLinecap="round"
            />
          </svg>
          <span>Reson</span>
        </div>
        <nav>
          {[
            { page: "home", label: "Home", icon: HomeIcon },
            { page: "search", label: "Search", icon: SearchIcon },
            { page: "library", label: "Library", icon: LibraryIcon },
            { page: "playlists", label: "Playlists", icon: ListMusic },
          ].map((item) => (
            <button
              key={item.page}
              className={route.page === item.page ? "selected" : ""}
              onClick={() =>
                navigate({
                  page: item.page as
                    | "home"
                    | "search"
                    | "library"
                    | "playlists",
                })
              }
            >
              <item.icon size={19} />
              {item.label}
            </button>
          ))}
        </nav>
        <div className="source-label">Sources</div>
        <>
          {sources.map((source) => (
            <button
              key={source.id}
              className={`source-button ${selectedSource === source.id ? "selected" : ""}`}
              onClick={() => {
                selectedProvider.set(source.id);
                navigate({ page: "home" });
              }}
            >
              <Cloud size={19} />
              <span>{source.display_name}</span>
              <i />
            </button>
          ))}
        </>
        <div className="sidebar-bottom">
          <p>
            One home.
            <br />
            Every source.
          </p>
          <button
            className={route.page === "settings" ? "selected" : ""}
            onClick={() => navigate({ page: "settings" })}
          >
            <SettingsIcon size={18} />
            Settings
          </button>
        </div>
      </aside>
      <main className="main-area">
        <div className="topbar">
          <div className="history-buttons">
            <button
              className="icon"
              title="Back"
              disabled={cursor === 0}
              onClick={() => setCursor((c) => c - 1)}
            >
              <ArrowLeft size={18} />
            </button>
            <button
              className="icon"
              title="Forward"
              disabled={cursor === routes.length - 1}
              onClick={() => setCursor((c) => c + 1)}
            >
              <ArrowRight size={18} />
            </button>
          </div>
          <span>
            {online ? (
              "Local-first music player"
            ) : (
              <>
                <WifiOff size={14} />
                Offline · your library is available
              </>
            )}
          </span>
          <button
            className="guest-button"
            onClick={() => navigate({ page: "settings" })}
          >
            <span>G</span>Guest
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
          ) : route.page === "library" ? (
            <LibraryView key="library" navigate={navigate} />
          ) : route.page === "playlists" ? (
            <LibraryView key="playlists" navigate={navigate} playlistsOnly />
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
      {notice && (
        <div className="toast" role="status">
          <span>{notice.text}</span>
          <button
            className="icon"
            title="Dismiss"
            onClick={() => notifyClear(notice.id)}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
import { notices } from "../stores/core";
function notifyClear(id: number) {
  if (notices.get()?.id === id) notices.set(null);
}
