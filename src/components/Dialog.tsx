import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

export function Dialog({
  title,
  close,
  children,
}: {
  title: string;
  close: () => void;
  children: ReactNode;
}) {
  const root = useRef<HTMLDivElement>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const node = root.current!;
    const focusable = () =>
      Array.from(
        node.querySelectorAll<HTMLElement>(
          'input:not(:disabled),button:not(:disabled),select:not(:disabled),[tabindex="0"]',
        ),
      );
    (
      node.querySelector<HTMLElement>("input:not(:disabled)") ??
      focusable()[0] ??
      node
    ).focus();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        closeRef.current();
      }
      if (e.key === "Tab") {
        const items = focusable();
        const first = items[0],
          last = items[items.length - 1];
        if (
          e.shiftKey &&
          (document.activeElement === first || document.activeElement === node)
        ) {
          e.preventDefault();
          last?.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };
    node.addEventListener("keydown", key);
    return () => {
      node.removeEventListener("keydown", key);
      previous?.focus();
    };
  }, []);
  return createPortal(
    <div
      className="modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        ref={root}
        tabIndex={-1}
      >
        <header>
          <h2>{title}</h2>
          <button
            type="button"
            className="icon"
            aria-label="Close dialog"
            onClick={close}
          >
            <X size={18} />
          </button>
        </header>
        {children}
      </div>
    </div>,
    document.body,
  );
}
