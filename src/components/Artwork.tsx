import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Music2 } from "lucide-react";
import { call } from "../stores/core";
const resolved = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();
export function Artwork({
  url,
  className = "",
}: {
  url: string | null;
  className?: string;
}) {
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
          if (alive) setSrc(url);
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
          onError={() => setSrc(null)}
        />
      ) : (
        <Music2 size={23} />
      )}
    </div>
  );
}
