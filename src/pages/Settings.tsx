import { useEffect, useState } from "react";
import { ExternalLink, FileText } from "lucide-react";
import {
  act,
  call,
  notify,
  original,
  useProviders,
  useSettings,
} from "../stores/core";
import type { Settings } from "../stores/types";
export function SettingsView() {
  const settings = useSettings();
  const providers = useProviders();
  const [cache, setCache] = useState(0);
  const [devices, setDevices] = useState<{ id: string; name: string }[]>([
    { id: "auto", name: "System default" },
  ]);
  const [discordId, setDiscordId] = useState(settings.discord_application_id);
  const [status, setStatus] = useState("");
  useEffect(() => {
    void call<number>("cache_size")
      .then(setCache)
      .catch(() => {});
    void call<{ id: string; name: string }[]>("audio_devices")
      .then(setDevices)
      .catch(() => {});
  }, []);
  const update = (patch: Partial<Settings>) =>
    act("update_settings", { settings: { ...settings, ...patch } });
  const exportLogs = async () => {
    try {
      setStatus(await call<string>("export_diagnostics"));
      notify("Diagnostics saved. Open the path shown in Settings.");
    } catch (e) {
      notify(String(e));
    }
  };
  return (
    <div className="page">
      <header className="page-title">
        <div>
          <h1>Settings</h1>
        </div>
      </header>
      <div className="settings-scroll">
        <section className="settings-section">
          <h2>Audio</h2>
          <div className="setting-row">
            <div>
              <strong>Output device</strong>
              <p>Use the system default or choose a device.</p>
            </div>
            <select
              aria-label="Audio output"
              value={settings.audio_device}
              onChange={(e) => update({ audio_device: e.target.value })}
            >
              {devices.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </div>
          <p className="setting-note">
            Minimize to keep listening. Closing the window exits Reson.
          </p>
        </section>
        <section className="settings-section">
          <h2>Music sources</h2>
          {providers.map((p) => (
            <div className="setting-row" key={p.id}>
              <div>
                <strong>{p.display_name}</strong>
                <p>Public tracks, artists, playlists and profile imports.</p>
              </div>
              <span className="connection-state">Public access</span>
            </div>
          ))}
          <p className="setting-note">
            Likes and playlists are saved in Reson. They do not change your
            SoundCloud account.
          </p>
        </section>
        <section className="settings-section">
          <h2>Discord Rich Presence</h2>
          <div className="setting-row">
            <div>
              <strong>Share listening activity with Discord</strong>
              <p>Show the current track in your Discord profile.</p>
            </div>
            <input
              type="checkbox"
              aria-label="Discord Rich Presence"
              checked={settings.discord_presence}
              disabled={!settings.discord_application_id}
              onChange={(e) => update({ discord_presence: e.target.checked })}
            />
          </div>
          <div className="setting-row">
            <div>
              <strong>Discord application ID</strong>
              <p>Enter an application ID to enable listening activity.</p>
            </div>
            <form
              className="inline-form"
              onSubmit={(e) => {
                e.preventDefault();
                update({ discord_application_id: discordId.trim() });
              }}
            >
              <input
                aria-label="Discord application ID"
                value={discordId}
                onChange={(e) => setDiscordId(e.target.value)}
                placeholder="Application ID"
              />
              <button className="secondary">Save</button>
            </form>
          </div>
        </section>
        <section className="settings-section">
          <h2>Cache</h2>
          <div className="setting-row">
            <div>
              <strong>Artwork cache</strong>
              <p>{(cache / 1024 / 1024).toFixed(1)} MB on this device.</p>
            </div>
            <button
              className="secondary"
              onClick={() => {
                void call("clear_cache")
                  .then(() => setCache(0))
                  .catch((e) => notify(String(e)));
              }}
            >
              Clear cache
            </button>
          </div>
          <div className="setting-row">
            <div>
              <strong>Maximum cache size</strong>
              <p>Older artwork is removed automatically.</p>
            </div>
            <select
              aria-label="Cache limit"
              value={settings.cache_limit_mb}
              onChange={(e) =>
                update({ cache_limit_mb: Number(e.target.value) })
              }
            >
              {[32, 64, 128, 256, 512, 1024, 2048].map((n) => (
                <option key={n} value={n}>
                  {n} MB
                </option>
              ))}
            </select>
          </div>
        </section>
        <section className="settings-section">
          <h2>Privacy</h2>
          <p className="setting-note">
            Your library and listening history stay on this device. Music
            requests go to SoundCloud. Reson does not collect usage telemetry.
          </p>
        </section>
        <section className="settings-section">
          <h2>About</h2>
          <div className="setting-row">
            <div>
              <strong>Reson 0.2.0</strong>
              <p>MIT · Windows desktop music player</p>
            </div>
            <button
              className="secondary"
              onClick={() => original("https://github.com/atlasru/reson")}
            >
              <ExternalLink size={15} />
              Repository
            </button>
          </div>
          <div className="setting-row">
            <div>
              <strong>Diagnostics</strong>
              <p>
                Provider status, audio and storage errors. Tokens and stream
                URLs excluded.
              </p>
            </div>
            <button className="secondary" onClick={() => void exportLogs()}>
              <FileText size={15} />
              Export
            </button>
          </div>
          {status && <p className="diagnostic-path">{status}</p>}
          <p className="setting-note">
            Space: play/pause · Alt ←/→: previous/next · Ctrl K: search · Ctrl
            Q: queue · Ctrl ↑/↓: volume
          </p>
        </section>
      </div>
    </div>
  );
}
