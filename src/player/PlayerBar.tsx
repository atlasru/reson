import { useEffect, useMemo, useRef, useState } from "react";
import {
  Heart,
  ListMusic,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
  LoaderCircle,
} from "lucide-react";
import { Artwork } from "../components/Artwork";
import { time } from "../components/TrackList";
import {
  control,
  favorite,
  useLibrary,
  usePlayer,
  useQueue,
} from "../stores/core";
import type { Route } from "../stores/types";
export function PlayerBar({
  queueOpen,
  onQueue,
  navigate,
}: {
  queueOpen: boolean;
  onQueue: () => void;
  navigate: (r: Route) => void;
}) {
  const p = usePlayer();
  const lib = useLibrary();
  const queue = useQueue();
  const [seek, setSeek] = useState<number | null>(null);
  const [volume, setVolume] = useState<number | null>(null);
  const volumeTimer = useRef<number | undefined>(undefined);
  const lastVolume = useRef(0.7);
  const t = p.current;
  const active = p.status === "playing" || p.status === "buffering";
  const busy = p.status === "loading" || p.status === "buffering";
  const likes = useMemo(
    () => new Set(lib.favorites.map((f) => f.internal_id)),
    [lib.favorites],
  );
  const liked = !!t && likes.has(t.internal_id);
  useEffect(() => () => window.clearTimeout(volumeTimer.current), []);
  useEffect(() => {
    if (p.volume > 0) lastVolume.current = p.volume;
  }, [p.volume]);
  const changeVolume = (value: number) => {
    setVolume(value);
    if (value > 0) lastVolume.current = value;
    window.clearTimeout(volumeTimer.current);
    volumeTimer.current = window.setTimeout(() => {
      control("volume", value);
      setVolume(null);
    }, 120);
  };
  return (
    <footer className="player-bar">
      <div className="now-playing">
        {t ? (
          <>
            <button
              className="now-track"
              onClick={() => navigate({ page: "track", track: t })}
            >
              <Artwork url={t.artwork} thumbnail />
              <span>
                <strong>{t.title}</strong>
                <small>{t.artists.map((a) => a.name).join(", ")}</small>
              </span>
            </button>
            <button
              className={`icon ${liked ? "active" : ""}`}
              title={liked ? "Remove from Reson likes" : "Like in Reson"}
              onClick={() => favorite(t.internal_id, !liked)}
            >
              <Heart size={18} fill={liked ? "currentColor" : "none"} />
            </button>
          </>
        ) : (
          <>
            <Artwork url={null} />
            <span>
              <strong>Ready when you are</strong>
              <small>Find something to listen to</small>
            </span>
          </>
        )}
      </div>
      <div className="playback-controls">
        <div className="transport">
          <button
            className={`icon ${p.shuffle ? "active" : ""}`}
            title="Shuffle"
            aria-pressed={p.shuffle}
            onClick={() => control("shuffle", !p.shuffle)}
          >
            <Shuffle size={16} />
          </button>
          <button
            className="icon"
            title="Previous · Alt ←"
            disabled={!t}
            onClick={() => control("previous")}
          >
            <SkipBack size={20} fill="currentColor" />
          </button>
          <button
            className="play-button"
            aria-label={active ? "Pause" : "Play"}
            title={active ? "Pause · Space" : "Play · Space"}
            disabled={!t && queue.entries.length === 0}
            onClick={() => control("toggle")}
          >
            {busy ? (
              <LoaderCircle size={22} className="spin" />
            ) : active ? (
              <Pause size={22} fill="currentColor" />
            ) : (
              <Play size={22} fill="currentColor" />
            )}
          </button>
          <button
            className="icon"
            title="Next · Alt →"
            disabled={!t}
            onClick={() => control("next")}
          >
            <SkipForward size={20} fill="currentColor" />
          </button>
          <button
            className={`icon ${p.repeat !== "off" ? "active" : ""}`}
            title={`Repeat: ${p.repeat}`}
            onClick={() =>
              control(
                "repeat",
                p.repeat === "off"
                  ? "queue"
                  : p.repeat === "queue"
                    ? "track"
                    : "off",
              )
            }
          >
            {p.repeat === "track" ? (
              <Repeat1 size={16} />
            ) : (
              <Repeat size={16} />
            )}
          </button>
        </div>
        <div className="progress">
          <span>{time(seek ?? p.position_ms)}</span>
          <input
            aria-label="Playback position"
            type="range"
            min={0}
            max={p.duration_ms || 1}
            value={seek ?? Math.min(p.position_ms, p.duration_ms)}
            disabled={!t}
            step={1000}
            onChange={(e) => setSeek(Number(e.target.value))}
            onPointerUp={(e) => {
              control("seek", Number(e.currentTarget.value));
              setSeek(null);
            }}
            onKeyUp={(e) => {
              control("seek", Number(e.currentTarget.value));
              setSeek(null);
            }}
            style={
              {
                "--progress": `${p.duration_ms ? (100 * (seek ?? p.position_ms)) / p.duration_ms : 0}%`,
              } as React.CSSProperties
            }
          />
          <span>{time(p.duration_ms)}</span>
        </div>
      </div>
      <div className="player-right">
        <button
          className={`icon ${queueOpen ? "active" : ""}`}
          title="Queue · Ctrl Q"
          onClick={onQueue}
        >
          <ListMusic size={20} />
        </button>
        <button
          className="icon"
          title="Mute"
          onClick={() => changeVolume(p.volume > 0 ? 0 : lastVolume.current)}
        >
          {(volume ?? p.volume) > 0 ? (
            <Volume2 size={19} />
          ) : (
            <VolumeX size={19} />
          )}
        </button>
        <input
          aria-label="Volume"
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={volume ?? p.volume}
          onChange={(e) => changeVolume(Number(e.target.value))}
          style={
            {
              "--progress": `${100 * (volume ?? p.volume)}%`,
            } as React.CSSProperties
          }
        />
      </div>
      {p.error && (
        <div className="playback-error" role="status">
          {p.error}
        </div>
      )}
    </footer>
  );
}
