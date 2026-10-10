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
        if (!node.contains(document.activeElement)) {
          e.preventDefault();
          (e.shiftKey ? last : first)?.focus();
        } else if (
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
    // Starting an import disables its focused input. Keep Escape/Tab working
    // even when the WebView temporarily moves focus to the document body.
    const retainFocus = () => {
      const active = document.activeElement;
      if (
        !node.contains(active) ||
        (active instanceof HTMLInputElement && active.disabled) ||
        (active instanceof HTMLButtonElement && active.disabled)
      )
        (focusable()[0] ?? node).focus();
    };
    const observer = new MutationObserver(retainFocus);
    observer.observe(node, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["disabled"],
    });
    window.addEventListener("keydown", key, true);
    window.addEventListener("focusin", retainFocus, true);
    return () => {
      observer.disconnect();
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("focusin", retainFocus, true);
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
