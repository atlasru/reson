import { expect, it, vi } from "vitest";
const bridge = vi.hoisted(() => ({
  callbacks: new Map<string, (event: { payload: unknown }) => void>(),
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: bridge.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (name: string, callback: (event: { payload: unknown }) => void) => {
      bridge.callbacks.set(name, callback);
      return () => bridge.callbacks.delete(name);
    },
  ),
}));
import { initialize, player, queue, library, settings } from "./core";

it("preserves playback volume when later settings edits use the shared snapshot", async () => {
  bridge.invoke.mockResolvedValue({
    player: player.get(),
    queue: queue.get(),
    library: library.get(),
    settings: settings.get(),
    providers: [],
    installation: null,
    audio_error: null,
  });
  const cleanup = await initialize();
  bridge.callbacks.get("player-state")!({
    payload: { ...player.get(), volume: 0.3 },
  });
  expect(player.get().volume).toBe(0.3);
  expect({ ...settings.get(), cache_limit_mb: 64 }.volume).toBe(0.3);
  cleanup();
});
