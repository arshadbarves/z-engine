import { useSyncExternalStore } from "react";
import {
  ArrowRight,
  CheckCircle2,
  Download,
  ExternalLink,
  LoaderCircle,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { LogoMark } from "../LogoMark";
import type { HarnessConfig } from "../../lib/commands";
import { updateStore } from "../../lib/updateStore";

function fmtBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${Math.round(bytes / 1024)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function AboutTab({ cfg }: { cfg: HarnessConfig }) {
  const { info, checking, installing, progress } = useSyncExternalStore(
    updateStore.subscribe,
    () => updateStore.getSnapshot(),
  );

  const pct = progress?.percentage != null ? Math.round(progress.percentage) : null;
  const isInstalling =
    installing && (progress?.phase === "installing" || progress?.phase === "ready");

  return (
    <div className="tab-body about-tab">
      <div className="about-hero">
        <LogoMark size={44} />
        <div className="about-hero-text">
          <h3>Z Engine</h3>
          <p className="form-note">
            The Autonomous AI Coding Engine · v{cfg.version ?? "1.3.0"}
          </p>
        </div>
      </div>

      {info?.available ? (
        <div className="settings-update-card" role="status">
          <div className="settings-update-head">
            <div className="settings-update-badge">
              <Sparkles size={15} />
            </div>
            <div className="settings-update-info">
              <div className="settings-update-title">Update Available</div>
              <div className="settings-update-versions">
                <span className="ver-current">v{info.current}</span>
                <ArrowRight size={11} className="ver-arrow" />
                <span className="ver-target">v{info.latest}</span>
              </div>
            </div>
            <button
              type="button"
              className="settings-update-primary-btn"
              disabled={installing}
              onClick={() => void updateStore.install()}
            >
              {installing ? (
                <>
                  <LoaderCircle size={13} className="spin" />
                  <span>Installing…</span>
                </>
              ) : (
                <>
                  <Download size={13} />
                  <span>Update & Restart</span>
                </>
              )}
            </button>
          </div>

          {info.releaseNotes && (
            <div className="settings-update-notes">
              <div className="settings-notes-label">What's New in v{info.latest}</div>
              <div className="settings-notes-content">{info.releaseNotes}</div>
            </div>
          )}

          {installing && (
            <div className="settings-update-progress">
              <div className="settings-progress-labels">
                <span className="settings-phase-text">
                  {isInstalling
                    ? "Applying update & restarting…"
                    : "Downloading update package…"}
                </span>
                <span className="settings-pct-text">{pct != null ? `${pct}%` : ""}</span>
              </div>
              <div className="settings-progress-track">
                <div
                  className={`settings-progress-bar${
                    pct == null ? " indeterminate" : ""
                  }`}
                  style={{ width: `${pct ?? 30}%` }}
                />
              </div>
              {progress?.totalBytes != null && progress.totalBytes > 0 && (
                <div className="settings-bytes-counter">
                  {fmtBytes(progress.downloadedBytes)} / {fmtBytes(progress.totalBytes)}
                </div>
              )}
            </div>
          )}

          {info.url && (
            <div className="settings-update-footer">
              <button
                type="button"
                className="settings-github-link"
                onClick={() => updateStore.openRelease()}
              >
                <ExternalLink size={12} />
                <span>View Release on GitHub</span>
              </button>
            </div>
          )}
        </div>
      ) : (
        <div className="settings-uptodate-card">
          <div className="uptodate-left">
            <CheckCircle2 size={16} className="uptodate-icon" />
            <div className="uptodate-text">
              <strong>Z Engine is up to date</strong>
              <span>Version {cfg.version ?? "1.3.0"} is the latest version available.</span>
            </div>
          </div>
          <button
            type="button"
            className="update-check"
            disabled={checking}
            onClick={() => void updateStore.check(true)}
          >
            <RefreshCw size={12} className={checking ? "spin" : undefined} />
            {checking ? "Checking…" : "Check Now"}
          </button>
        </div>
      )}

      <div className="about-section-divider" />

      <h4 className="about-paths-title">System Paths & Configuration</h4>
      <dl className="about-dl">
        <dt>Global Config</dt>
        <dd>
          <code>~/.config/z-engine/config.toml</code>
          <span className="form-note"> created on first launch · API key in auth.json</span>
        </dd>
        <dt>Project Config</dt>
        <dd>
          <code>.z-engine/config.toml</code>
        </dd>
        <dt>Session Store</dt>
        <dd>
          <code>Application Support/z-engine/sessions</code>
        </dd>
        <dt>Active Model</dt>
        <dd>
          <code>{cfg.model}</code>
        </dd>
      </dl>
    </div>
  );
}
