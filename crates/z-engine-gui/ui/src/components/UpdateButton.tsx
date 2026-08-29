import { useSyncExternalStore } from "react";
import { ExternalLink, LoaderCircle, Sparkles } from "lucide-react";
import { updateStore } from "../lib/updateStore";

/** One-click inline update button without intrusive popups. */
export function UpdateButton() {
  const { info, installing, progress } = useSyncExternalStore(
    updateStore.subscribe,
    () => updateStore.getSnapshot(),
  );

  if (!info?.available) return null;

  const pct = progress?.percentage != null ? Math.round(progress.percentage) : null;
  const isDownloading = installing && progress?.phase === "downloading";
  const isInstalling =
    installing && (progress?.phase === "installing" || progress?.phase === "ready");

  const label = isInstalling
    ? "Installing…"
    : isDownloading
      ? pct != null
        ? `${pct}%`
        : "Downloading…"
      : `Update v${info.latest}`;

  return (
    <div className="update-btn-wrap">
      <button
        type="button"
        className={`update-header-pill${installing ? " installing" : ""}`}
        title={
          installing
            ? "Downloading and installing update…"
            : `Click to directly update to v${info.latest}`
        }
        disabled={installing}
        onClick={() => void updateStore.install()}
      >
        {installing ? (
          <LoaderCircle size={12} className="spin update-spin-icon" strokeWidth={2} />
        ) : (
          <Sparkles size={12} className="update-sparkle-icon" strokeWidth={1.8} />
        )}
        <span className="update-header-text">{label}</span>
        {installing && pct != null && (
          <span
            className="update-pill-progress-bar"
            style={{ width: `${pct}%` }}
            aria-hidden="true"
          />
        )}
      </button>

      {info.url && !installing && (
        <button
          type="button"
          className="update-fallback-icon-btn"
          title="Download manually from GitHub Releases"
          onClick={() => updateStore.openRelease()}
        >
          <ExternalLink size={11} strokeWidth={1.8} />
        </button>
      )}
    </div>
  );
}

