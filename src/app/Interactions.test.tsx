import {
  act as reactAct,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
const bridge = vi.hoisted(() => ({
  invoke: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  scroll: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: bridge.invoke,
  convertFileSrc: (p: string) => p,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (name: string, callback: (event: { payload: unknown }) => void) => {
      bridge.listeners.set(name, callback);
      return () => bridge.listeners.delete(name);
    },
  ),
}));
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 48,
    getVirtualItems: () =>
      Array.from({ length: Math.min(count, 20) }, (_, i) => ({
        index: i,
        key: i,
        start: i * 48,
        size: 48,
      })),
    scrollToIndex: bridge.scroll,
  }),
}));
import { App } from "./App";
import { TrackList } from "../components/TrackList";
import { ImportDialog } from "../library/ImportDialog";
import { Dialog } from "../components/Dialog";
import {
  imports,
  library,
  player,
  queue,
  settings,
  providers,
  notices,
} from "../stores/core";
import type { ImportProgress, Track } from "../stores/types";
const track: Track = {
  internal_id: "track-1",
  title: "First song",
  artists: [
    {
      internal_id: "artist-1",
      name: "Creator",
      artwork: null,
      description: null,
      references: [
        {
          provider: "soundcloud",
          provider_id: "soundcloud:users:1",
          url: "https://soundcloud.com/creator",
        },
      ],
    },
  ],
  artwork: null,
  album: null,
  duration_ms: 180000,
  explicit: false,
  availability: "playable",
  sources: [
    {
      provider: "soundcloud",
      provider_id: "soundcloud:tracks:1",
      url: "https://soundcloud.com/creator/track",
      availability: "playable",
    },
  ],
};
const progress: ImportProgress = {
  job_id: "job-1",
  source_id: "source-1",
  profile: "Creator",
  status: "importing",
  discovered: 100,
  saved: 80,
  duplicates: 19,
  failed: 1,
  unavailable: 3,
  pages: 1,
  retry_at: null,
  message: null,
};
const empty = () => ({
  favorites: [],
  recent: [],
  playlists: [],
  local_favorite_ids: [],
  import_sources: [],
  import_track_sources: {},
});
beforeEach(() => {
  bridge.invoke.mockReset();
  bridge.scroll.mockReset();
  imports.set([]);
  library.set(empty());
  notices.set(null);
  localStorage.clear();
  providers.set([
    {
      id: "soundcloud",
      display_name: "SoundCloud",
      mode: "guest",
      capabilities: ["search", "discovery", "public_likes_import"],
    },
  ]);
  bridge.invoke.mockImplementation(async (name: string) => {
    if (name === "bootstrap")
      return {
        player: player.get(),
        queue: queue.get(),
        settings: settings.get(),
        library: library.get(),
        providers: providers.get(),
        imports: [],
        installation: null,
        audio_error: null,
      };
    if (name === "discover") return [track];
    if (name === "search")
      return { tracks: [track], artists: [], playlists: [], has_more: false };
    if (name === "start_likes_import") return progress;
    if (name === "library_state") return library.get();
    if (name === "audio_devices")
      return [{ id: "auto", name: "System default" }];
    if (name === "cache_size") return 0;
    return undefined;
  });
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("desktop navigation and shortcuts", () => {
  it("navigates library/likes/settings and preserves the player", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Home", level: 1 });
    fireEvent.click(screen.getByRole("button", { name: "Liked Tracks" }));
    expect(
      screen.getByRole("heading", { name: "Liked Tracks", level: 1 }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(
      screen.getByRole("heading", { name: "Settings", level: 1 }),
    ).toBeInTheDocument();
    expect(document.querySelectorAll(".player-bar")).toHaveLength(1);
    expect(
      bridge.invoke.mock.calls.some(([name]) => name === "player_control"),
    ).toBe(false);
  });
  it("search shortcut focuses search and displays results", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Home", level: 1 });
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    const input = screen.getByRole("textbox", { name: "Search music" });
    expect(input).toHaveFocus();
    fireEvent.change(input, { target: { value: "Creator" } });
    await screen.findByText("First song");
    expect(bridge.invoke).toHaveBeenCalledWith(
      "search",
      expect.objectContaining({ query: "Creator", provider: "soundcloud" }),
    );
  });
  it("space toggles playback but typing and dialogs do not", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Home", level: 1 });
    fireEvent.keyDown(window, { code: "Space" });
    expect(bridge.invoke).toHaveBeenCalledWith("player_control", {
      action: "toggle",
      value: null,
    });
    bridge.invoke.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Liked Tracks" }));
    fireEvent.click(
      screen.getAllByRole("button", {
        name: "Import from SoundCloud",
      })[0],
    );
    fireEvent.keyDown(
      screen.getByRole("textbox", { name: "SoundCloud profile" }),
      { code: "Space" },
    );
    expect(
      bridge.invoke.mock.calls.some(([name]) => name === "player_control"),
    ).toBe(false);
  });
  it("sidebar collapses and the queue opens with its shortcut", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Home", level: 1 });
    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));
    expect(document.querySelector(".app-shell")).toHaveClass(
      "sidebar-collapsed",
    );
    fireEvent.keyDown(window, { key: "q", ctrlKey: true });
    expect(screen.getByRole("heading", { name: "Queue" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(
      screen.queryByRole("heading", { name: "Queue" }),
    ).not.toBeInTheDocument();
  });
});
describe("import interaction", () => {
  it("submits a profile and displays truthful counters", async () => {
    render(<ImportDialog close={() => {}} />);
    fireEvent.change(
      screen.getByRole("textbox", { name: "SoundCloud profile" }),
      { target: { value: "creator" } },
    );
    fireEvent.submit(document.querySelector(".dialog form")!);
    await screen.findByText("Importing liked tracks…");
    expect(bridge.invoke).toHaveBeenCalledWith("start_likes_import", {
      provider: "soundcloud",
      input: "creator",
    });
    expect(screen.getByText("80")).toBeInTheDocument();
    expect(screen.getByText("19")).toBeInTheDocument();
    expect(screen.getByText("Failed")).toBeInTheDocument();
  });
  it("cancels requests and long cooldowns without waiting", async () => {
    imports.set([
      {
        ...progress,
        status: "cooldown",
        retry_at: Math.floor(Date.now() / 1000) + 1800,
        message: "Rate limit",
      },
    ]);
    render(<ImportDialog initialJob="job-1" close={() => {}} />);
    expect(screen.getByText(/Retrying in/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel import" }));
    await waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith("cancel_likes_import", {
        jobId: "job-1",
      }),
    );
    reactAct(() =>
      imports.set([
        {
          ...progress,
          status: "cancelled",
          message: "Saved tracks were kept.",
        },
      ]),
    );
    expect(screen.getByText("Import cancelled")).toBeInTheDocument();
    expect(screen.getByText("80")).toBeInTheDocument();
  });
  it("partial results are never labelled complete and input errors remain visible", async () => {
    imports.set([
      {
        ...progress,
        status: "partial",
        message: "Connection failed after first page.",
      },
    ]);
    render(<ImportDialog initialJob="job-1" close={() => {}} />);
    expect(screen.getByText("Partial import")).toBeInTheDocument();
    expect(screen.queryByText("Import complete")).not.toBeInTheDocument();
    cleanup();
    bridge.invoke.mockRejectedValueOnce("Invalid input: unsupported domain");
    render(<ImportDialog close={() => {}} />);
    fireEvent.change(
      screen.getByRole("textbox", { name: "SoundCloud profile" }),
      { target: { value: "https://evil.test/a" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Invalid input: unsupported domain",
    );
  });
  it("Escape closes the dialog and focus returns to its trigger", () => {
    const close = vi.fn();
    const view = render(
      <>
        <button>Trigger</button>
        <Dialog title="Test" close={close}>
          <input aria-label="Name" />
        </Dialog>
      </>,
    );
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Name" }), {
      key: "Escape",
    });
    expect(close).toHaveBeenCalledOnce();
    view.unmount();
  });
});
describe("track interactions", () => {
  it("single click selects; double click plays; arrow and Enter navigate", () => {
    const navigate = vi.fn();
    render(
      <TrackList
        tracks={[track, { ...track, internal_id: "track-2", title: "Second" }]}
        navigate={navigate}
      />,
    );
    fireEvent.click(screen.getByText("First song"));
    expect(navigate).not.toHaveBeenCalled();
    expect(bridge.invoke).not.toHaveBeenCalled();
    fireEvent.doubleClick(screen.getByText("First song"));
    expect(bridge.invoke).toHaveBeenCalledWith("play_tracks", {
      ids: ["track-1", "track-2"],
      index: 0,
    });
    const list = screen.getByRole("listbox");
    fireEvent.keyDown(list, { key: "ArrowDown" });
    expect(bridge.scroll).toHaveBeenCalledWith(1, { align: "auto" });
    fireEvent.keyDown(list, { key: "Enter" });
    expect(bridge.invoke).toHaveBeenCalledWith("play_tracks", {
      ids: ["track-1", "track-2"],
      index: 1,
    });
  });
  it("context menu supports play-next, queue and opening track/artist", () => {
    const navigate = vi.fn();
    render(<TrackList tracks={[track]} navigate={navigate} />);
    fireEvent.contextMenu(screen.getByText("First song"), {
      clientX: 200,
      clientY: 100,
    });
    fireEvent.click(screen.getByRole("menuitem", { name: "Play next" }));
    expect(bridge.invoke).toHaveBeenCalledWith("enqueue_tracks", {
      ids: ["track-1"],
      next: true,
    });
    fireEvent.contextMenu(screen.getByText("First song"));
    fireEvent.click(screen.getByRole("menuitem", { name: "Open track" }));
    expect(navigate).toHaveBeenCalledWith({ page: "track", track });
    fireEvent.contextMenu(screen.getByText("First song"));
    fireEvent.click(screen.getByRole("menuitem", { name: "Add to queue" }));
    expect(bridge.invoke).toHaveBeenCalledWith("enqueue_tracks", {
      ids: ["track-1"],
      next: false,
    });
    fireEvent.click(screen.getByRole("button", { name: "Creator" }));
    expect(navigate).toHaveBeenCalledWith({
      page: "artist",
      provider: "soundcloud",
      id: "soundcloud:users:1",
    });
  });
  it("shows source provenance and sends local likes to storage", () => {
    library.set({
      ...empty(),
      favorites: [track],
      import_sources: [
        {
          id: "source-1",
          provider: "soundcloud",
          provider_user_id: "1",
          url: "https://soundcloud.com/profile",
          name: "Imported profile",
          artwork: null,
          created_at: 0,
          refreshed_at: null,
          track_count: 1,
          progress: null,
        },
      ],
      import_track_sources: { "track-1": ["source-1"] },
    });
    render(<TrackList tracks={[track]} showProvenance navigate={() => {}} />);
    expect(screen.getByText("From Imported profile")).toBeInTheDocument();
    fireEvent.click(screen.getByTitle("Remove from Reson likes"));
    expect(bridge.invoke).toHaveBeenCalledWith("set_favorite", {
      id: "track-1",
      enabled: false,
    });
  });
  it("a very large library renders only virtual rows", () => {
    render(
      <TrackList
        tracks={Array.from({ length: 12000 }, (_, i) => ({
          ...track,
          internal_id: String(i),
          title: `Track ${i}`,
        }))}
        navigate={() => {}}
      />,
    );
    expect(
      within(screen.getByRole("listbox")).getAllByRole("option"),
    ).toHaveLength(20);
  });
});
