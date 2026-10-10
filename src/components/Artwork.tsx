import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Music2 } from "lucide-react";
import { call } from "../stores/core";
const resolved = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();
export function Artwork({
  url,
  className = "",
  thumbnail = false,
}: {
  url: string | null;
  className?: string;
  thumbnail?: boolean;
}) {
  // SoundCloud's original 100px artwork is sufficient for compact desktop rows.
  // Other providers keep their own URLs unchanged.
  if (thumbnail && url) {
    try {
      if (new URL(url).hostname.endsWith(".sndcdn.com"))
        url = url.replace("-t500x500.", "-large.");
    } catch {
      url = null;
    }
  }
  const [src, setSrc] = useState(url ? (resolved.get(url) ?? null) : null);
  useEffect(() => {
    let alive = true;
    setSrc(url ? (resolved.get(url) ?? null) : null);
    if (url && !resolved.has(url)) {
      let request = inflight.get(url);
      if (!request) {
        request = call<string>("cached_artwork", { url })
          .then((path) => {
            const src = convertFileSrc(path);
            if (resolved.size >= 512)
              resolved.delete(resolved.keys().next().value!);
            resolved.set(url, src);
            return src;
          })
          .finally(() => inflight.delete(url));
        inflight.set(url, request);
      }
      void request
        .then((value) => {
          if (alive) setSrc(value);
        })
        .catch(() => {
          if (alive) setSrc(null);
        });
    }
    return () => {
      alive = false;
    };
  }, [url]);
  return (
    <div className={`artwork ${className}`}>
      {src ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          draggable={false}
          onError={() => {
            if (url) resolved.delete(url);
            setSrc(null);
          }}
        />
      ) : (
        <Music2 size={23} />
      )}
    </div>
  );
}
