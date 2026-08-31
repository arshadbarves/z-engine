import { useSyncExternalStore } from "react";
import { Check, Download, ExternalLink, LoaderCircle } from "../lib/icons";
import { updateStore } from "../lib/updateStore";

/** Luxury update action capsule with relatable download iconography and live progress. */
export function UpdateButton() {
  const { info, installing, progress } = useSyncExternalStore(
    updateStore.subscribe,
    () => updateStore.getSnapshot(),
  );

  if (!info?.available) return null;

  const pct = progress?.percentage != null ? Math.round(progress.percentage) : null;
  const isDownloading = installing && progress?.phase === "downloading";
  const isReady = installing && progress?.phase === "ready";
  const isInstalling = installing && (progress?.phase === "installing" || isReady);

  return (
    <div className="update-btn-wrap">
      <button
        type="button"
        className={`luxury-update-pill${installing ? " is-active" : ""}${isReady ? " is-ready" : ""}`}
        title={
          isReady
            ? "Update ready. Restart application to complete update."
            : installing
              ? `Downloading update${pct != null ? ` (${pct}%)` : "…"}`
              : `Directly update to v${info.latest}`
        }
        disabled={installing && !isReady}
        onClick={() => void updateStore.install()}
      >
        <span className="update-icon-wrap">
          {isReady ? (
            <Check size={12} strokeWidth={2.4} className="update-check-icon" />
          ) : installing ? (
            <LoaderCircle size={12} strokeWidth={2.2} className="spin update-spin-icon" />
          ) : (
            <Download size={12} strokeWidth={2} className="update-download-icon" />
          )}
        </span>

        <span className="update-label">
          {isReady
            ? "Restart to apply"
            : isInstalling
              ? "Installing…"
              : isDownloading
                ? pct != null
                  ? `Downloading ${pct}%`
                  : "Downloading…"
                : `v${info.latest}`}
        </span>

        {!installing && <span className="update-beacon-dot" aria-hidden="true" />}

        {installing && pct != null && (
          <span
            className="update-progress-fill"
            style={{ width: `${pct}%` }}
            aria-hidden="true"
          />
        )}
      </button>

      {info.url && !installing && (
        <button
          type="button"
          className="update-ext-btn"
          title="Open GitHub release details"
          onClick={() => updateStore.openRelease()}
          aria-label="Open GitHub release details"
        >
          <ExternalLink size={11} strokeWidth={1.8} />
        </button>
      )}
    </div>
  );
}

