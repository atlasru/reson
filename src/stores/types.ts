export type Availability = "playable" | "preview" | "unavailable";
export interface ProviderRef {
  provider: string;
  provider_id: string;
  url: string | null;
}
export interface Artist {
  internal_id: string;
  name: string;
  artwork: string | null;
  description: string | null;
  references: ProviderRef[];
}
export interface TrackSource extends ProviderRef {
  availability: Availability;
}
export interface Track {
  internal_id: string;
  title: string;
  artists: Artist[];
  album: string | null;
  artwork: string | null;
  duration_ms: number;
  explicit: boolean | null;
  availability: Availability;
  sources: TrackSource[];
}
export interface Playlist {
  internal_id: string;
  title: string;
  artwork: string | null;
  description: string | null;
  owner: Artist | null;
  tracks: Track[];
  track_count: number;
  reference: ProviderRef | null;
}
export interface SearchResults {
  tracks: Track[];
  artists: Artist[];
  playlists: Playlist[];
  has_more: boolean;
}
export interface ArtistPage {
  artist: Artist;
  tracks: Track[];
  playlists: Playlist[];
  has_more: boolean;
}
export type RepeatMode = "off" | "queue" | "track";
export interface QueueEntry {
  entry_id: string;
  track: Track;
}
export interface Queue {
  entries: QueueEntry[];
  current: string | null;
  shuffle: boolean;
  repeat: RepeatMode;
  order: string[];
}
export interface PlayerState {
  status: "stopped" | "loading" | "buffering" | "playing" | "paused" | "error";
  current: Track | null;
  entry_id: string | null;
  position_ms: number;
  duration_ms: number;
  volume: number;
  shuffle: boolean;
  repeat: RepeatMode;
  error: string | null;
}
export interface Library {
  favorites: Track[];
  recent: Track[];
  playlists: Playlist[];
}
export interface Settings {
  volume: number;
  cache_limit_mb: number;
  discord_presence: boolean;
  discord_application_id: string;
  telemetry: boolean;
  audio_device: string;
}
export interface ProviderInfo {
  id: string;
  display_name: string;
  capabilities: string[];
  mode: string;
}
export interface Installation {
  installation_id: string;
  platform: string;
  app_version: string;
  first_seen: number;
  last_seen: number;
}
export interface AppSnapshot {
  player: PlayerState;
  queue: Queue;
  library: Library;
  settings: Settings;
  providers: ProviderInfo[];
  installation: Installation;
  audio_error: string | null;
}
export type Route =
  | { page: "home" | "search" | "library" | "playlists" | "settings" }
  | { page: "artist"; provider: string; id: string }
  | { page: "playlist"; playlist: Playlist }
  | { page: "track"; track: Track };
