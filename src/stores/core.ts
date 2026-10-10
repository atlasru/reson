import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppSnapshot,
  Library,
  PlayerState,
  Queue,
  Settings,
  ProviderInfo,
  Installation,
  ImportProgress,
} from "./types";

function atom<T>(initial: T) {
  let value = initial;
  const listeners = new Set<() => void>();
  return {
    get: () => value,
    set: (next: T) => {
      value = next;
      listeners.forEach((l) => l());
    },
    subscribe: (fn: () => void) => {
      listeners.add(fn);
      return () => {
        listeners.delete(fn);
      };
    },
  };
}
export const player = atom<PlayerState>({
  status: "stopped",
  current: null,
  entry_id: null,
  position_ms: 0,
  duration_ms: 0,
  volume: 0.7,
  shuffle: false,
  repeat: "off",
  error: null,
});
export const queue = atom<Queue>({
  entries: [],
  current: null,
  shuffle: false,
  repeat: "off",
  order: [],
});
export const library = atom<Library>({
  favorites: [],
  recent: [],
  playlists: [],
  local_favorite_ids: [],
  import_sources: [],
  import_track_sources: {},
});
export const imports = atom<ImportProgress[]>([]);
export const useImports = () =>
  useSyncExternalStore(imports.subscribe, imports.get);
export const importActive = (p: ImportProgress) =>
  ["resolving", "importing", "cooldown"].includes(p.status);
export function rememberImport(p: ImportProgress) {
  // A fast job may finish before its start command reaches the renderer.
  const previous = imports.get().find((j) => j.job_id === p.job_id);
  if (previous && !importActive(previous) && importActive(p)) return;
  imports.set([...imports.get().filter((j) => j.job_id !== p.job_id), p]);
}
export const settings = atom<Settings>({
  volume: 0.7,
  cache_limit_mb: 256,
  discord_presence: false,
  discord_application_id: "",
  telemetry: false,
  audio_device: "auto",
});
export const providers = atom<ProviderInfo[]>([]);
export const selectedProvider = atom<string | null>(null);
export const useSelectedProvider = () =>
  useSyncExternalStore(selectedProvider.subscribe, selectedProvider.get);
export const installation = atom<Installation | null>(null);
export const notices = atom<{ id: number; text: string } | null>(null);
export const usePlayer = () =>
  useSyncExternalStore(player.subscribe, player.get);
export const useCurrentTrackId = () =>
  useSyncExternalStore(
    player.subscribe,
    () => player.get().current?.internal_id,
  );
export const useQueue = () => useSyncExternalStore(queue.subscribe, queue.get);
export const useLibrary = () =>
  useSyncExternalStore(library.subscribe, library.get);
export const useSettings = () =>
  useSyncExternalStore(settings.subscribe, settings.get);
export const useProviders = () =>
  useSyncExternalStore(providers.subscribe, providers.get);
export const useNotice = () =>
  useSyncExternalStore(notices.subscribe, notices.get);
export const notify = (text: string) => {
  notices.set({ id: Date.now(), text });
};
export async function call<T = void>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return invoke<T>(name, args);
}
export function act(name: string, args?: Record<string, unknown>) {
  void call(name, args).catch((e) => notify(String(e)));
}
export async function refreshLibrary() {
  library.set(await call<Library>("library_state"));
}
export async function initialize() {
  let refreshTimer: number | undefined;
  const updateLibrary = () => {
    if (refreshTimer !== undefined) return;
    refreshTimer = window.setTimeout(() => {
      refreshTimer = undefined;
      void refreshLibrary().catch((e) => notify(String(e)));
    }, 500);
  };
  const unlisten = await Promise.all([
    listen<PlayerState>("player-state", (e) => {
      player.set(e.payload);
      const current = settings.get();
      if (current.volume !== e.payload.volume) {
        settings.set({ ...current, volume: e.payload.volume });
      }
    }),
    listen<Queue>("queue-state", (e) => queue.set(e.payload)),
    listen<Library>("library-state", (e) => library.set(e.payload)),
    listen<Settings>("settings-state", (e) => settings.set(e.payload)),
    listen<ImportProgress>("import-progress", (e) => {
      rememberImport(e.payload);
      if (e.payload.pages > 0 || !importActive(e.payload)) updateLibrary();
    }),
  ]);
  let s: AppSnapshot;
  try {
    s = await call<AppSnapshot>("bootstrap");
  } catch (e) {
    unlisten.forEach((fn) => fn());
    throw e;
  }
  player.set(s.player);
  queue.set(s.queue);
  library.set(s.library);
  settings.set(s.settings);
  providers.set(s.providers);
  installation.set(s.installation);
  imports.set(s.imports ?? []);
  if (s.audio_error) notify(s.audio_error);
  return () => {
    window.clearTimeout(refreshTimer);
    unlisten.forEach((fn) => fn());
  };
}
export const play = (ids: string[], index = 0) =>
  act("play_tracks", { ids, index });
export const enqueue = (ids: string[], next = false) =>
  act("enqueue_tracks", { ids, next });
export const control = (action: string, value?: number | string | boolean) =>
  act("player_control", { action, value: value ?? null });
export const favorite = (id: string, enabled: boolean) =>
  act("set_favorite", { id, enabled });
export const original = (url: string) => act("open_external", { url });
