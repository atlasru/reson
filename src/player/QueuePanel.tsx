import { useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowDown,
  ArrowUp,
  GripVertical,
  Play,
  Trash2,
  X,
} from "lucide-react";
import { Artwork } from "../components/Artwork";
import { Empty } from "../components/States";
import { act, useQueue } from "../stores/core";

export function QueuePanel({ close }: { close: () => void }) {
  const queue = useQueue();
  const parent = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const virtual = useVirtualizer({
    count: queue.entries.length,
    getScrollElement: () => parent.current,
    estimateSize: () => 52,
    overscan: 5,
    getItemKey: (index) => queue.entries[index].entry_id,
  });
  return (
    <aside className="queue-panel">
      <header>
        <div>
          <h2>Queue</h2>
          <small>
            {queue.entries.length} tracks{queue.shuffle ? " · Shuffled" : ""}
          </small>
        </div>
        <button className="icon" title="Close queue" onClick={close}>
          <X size={18} />
        </button>
      </header>
      <div className="queue-list" ref={parent}>
        {queue.entries.length === 0 ? (
          <Empty
            title="Your queue is empty"
            detail="Add tracks from their action menus."
          />
        ) : (
          <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
            {virtual.getVirtualItems().map((row) => {
              const index = row.index;
              const entry = queue.entries[index];
              return (
                <div
                  key={entry.entry_id}
                  className={`queue-item ${entry.entry_id === queue.current ? "current" : ""} ${dragging === entry.entry_id ? "dragging" : ""}`}
                  style={{
                    position: "absolute",
                    left: 0,
                    top: 0,
                    width: "100%",
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                  }}
                  tabIndex={0}
                  draggable
                  onDragStart={(event) => {
                    setDragging(entry.entry_id);
                    event.dataTransfer.setData("text/plain", entry.entry_id);
                    event.dataTransfer.effectAllowed = "move";
                  }}
                  onDragEnd={() => setDragging(null)}
                  onDragOver={(event) => {
                    event.preventDefault();
                    event.dataTransfer.dropEffect = "move";
                  }}
                  onDrop={(event) => {
                    event.preventDefault();
                    const id = event.dataTransfer.getData("text/plain");
                    if (queue.entries.some((entry) => entry.entry_id === id))
                      act("queue_reorder", { entryId: id, to: index });
                    setDragging(null);
                  }}
                  onDoubleClick={() =>
                    act("queue_select", { id: entry.entry_id })
                  }
                >
                  <GripVertical size={14} className="grip" />
                  <Artwork url={entry.track.artwork} />
                  <div className="queue-title">
                    <strong>{entry.track.title}</strong>
                    <small>
                      {entry.track.artists
                        .map((artist) => artist.name)
                        .join(", ")}
                    </small>
                  </div>
                  <div className="queue-actions">
                    <button
                      className="icon"
                      title="Play queue entry"
                      onClick={() =>
                        act("queue_select", { id: entry.entry_id })
                      }
                    >
                      <Play size={13} />
                    </button>
                    <button
                      className="icon"
                      title="Move up"
                      disabled={index === 0}
                      onClick={() =>
                        act("queue_reorder", {
                          entryId: entry.entry_id,
                          to: index - 1,
                        })
                      }
                    >
                      <ArrowUp size={13} />
                    </button>
                    <button
                      className="icon"
                      title="Move down"
                      disabled={index === queue.entries.length - 1}
                      onClick={() =>
                        act("queue_reorder", {
                          entryId: entry.entry_id,
                          to: index + 1,
                        })
                      }
                    >
                      <ArrowDown size={13} />
                    </button>
                    <button
                      className="icon"
                      title="Remove queue entry"
                      onClick={() =>
                        act("queue_remove", { id: entry.entry_id })
                      }
                    >
                      <X size={13} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
      {queue.entries.length > 0 && (
        <button
          className="secondary clear-queue"
          onClick={() => act("queue_clear")}
        >
          <Trash2 size={14} />
          Clear queue
        </button>
      )}
    </aside>
  );
}
