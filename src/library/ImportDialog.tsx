import { useEffect, useState } from "react";
import { Check, LoaderCircle } from "lucide-react";
import { Dialog } from "../components/Dialog";
import { call, importActive, rememberImport, useImports } from "../stores/core";
import type { ImportProgress } from "../stores/types";

export function ImportSummary({ progress: p }: { progress: ImportProgress }) {
  return (
    <dl className="import-counts">
      <div>
        <dt>Discovered</dt>
        <dd>{p.discovered}</dd>
      </div>
      <div>
        <dt>Saved</dt>
        <dd>{p.saved}</dd>
      </div>
      <div>
        <dt>Duplicates skipped</dt>
        <dd>{p.duplicates}</dd>
      </div>
      <div>
        <dt>Failed</dt>
        <dd>{p.failed}</dd>
      </div>
    </dl>
  );
}
export function ImportDialog({
  close,
  initialInput = "",
  initialJob,
}: {
  close: () => void;
  initialInput?: string;
  initialJob?: string;
}) {
  const jobs = useImports();
  const [input, setInput] = useState(initialInput);
  const [job, setJob] = useState<string | undefined>(initialJob);
  const [error, setError] = useState("");
  const [starting, setStarting] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [now, setNow] = useState(Date.now());
  const progress = jobs.find((p) => p.job_id === job);
  const running = !!progress && importActive(progress);
  useEffect(() => {
    if (progress?.status !== "cooldown") return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [progress?.status]);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (running || starting) return;
    setStarting(true);
    setError("");
    setCancelling(false);
    try {
      const p = await call<ImportProgress>("start_likes_import", {
        provider: "soundcloud",
        input,
      });
      rememberImport(p);
      setJob(p.job_id);
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  };
  const cancel = async () => {
    if (!job || cancelling) return;
    setCancelling(true);
    try {
      await call("cancel_likes_import", { jobId: job });
    } catch (e) {
      setError(String(e));
      setCancelling(false);
    }
  };
  const status = progress?.status;
  const heading =
    status === "complete"
      ? "Import complete"
      : status === "partial"
        ? "Partial import"
        : status === "cancelled"
          ? "Import cancelled"
          : status === "failed"
            ? "Import failed"
            : status === "cooldown"
              ? `Retrying in ${Math.max(0, Math.ceil((progress!.retry_at! * 1000 - now) / 1000))}s`
              : status === "resolving"
                ? "Resolving profile…"
                : "Importing liked tracks…";
  return (
    <Dialog title="Import from SoundCloud" close={close}>
      <form onSubmit={(e) => void submit(e)}>
        <p className="dialog-description">
          Copy public liked tracks into your Reson library. No SoundCloud login
          needed.
        </p>
        <label className="field-label" htmlFor="soundcloud-profile">
          Profile URL or username
        </label>
        <input
          id="soundcloud-profile"
          aria-label="SoundCloud profile"
          placeholder="soundcloud.com/username or username"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          disabled={running || starting}
          autoComplete="off"
          spellCheck={false}
        />
        {progress && (
          <section
            className={`import-progress ${status}`}
            aria-live="polite"
            aria-label="Import progress"
          >
            <div className="import-status">
              {running ? (
                <LoaderCircle size={16} className="spin" />
              ) : status === "complete" ? (
                <Check size={16} />
              ) : null}
              <strong>{heading}</strong>
            </div>
            <p className="import-profile">{progress.profile}</p>
            {running && (
              <div
                className="indeterminate"
                role="progressbar"
                aria-label="Import in progress"
              />
            )}
            <ImportSummary progress={progress} />
            {progress.unavailable > 0 && (
              <p className="muted">
                {progress.unavailable} unavailable tracks saved with metadata.
              </p>
            )}
            {progress.message && (
              <p
                className={
                  status === "partial" || status === "failed"
                    ? "inline-error"
                    : "muted"
                }
              >
                {progress.message}
              </p>
            )}
          </section>
        )}
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        <p className="dialog-note">
          Likes stay on this device. Imported tracks never change the profile’s
          SoundCloud likes.
        </p>
        <footer className="dialog-actions">
          <button className="secondary" type="button" onClick={close}>
            {running ? "Continue in background" : "Close"}
          </button>
          {running ? (
            <button
              className="primary"
              type="button"
              disabled={cancelling}
              onClick={() => void cancel()}
            >
              {cancelling ? "Cancelling…" : "Cancel import"}
            </button>
          ) : (
            <button className="primary" disabled={starting || !input.trim()}>
              {starting ? "Starting…" : progress ? "Import again" : "Import"}
            </button>
          )}
        </footer>
      </form>
    </Dialog>
  );
}
