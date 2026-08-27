import { LogoMark } from "../LogoMark";
import type { HarnessConfig } from "../../lib/commands";

export function AboutTab({ cfg }: { cfg: HarnessConfig }) {
  return (
    <div className="tab-body about-tab">
      <LogoMark size={36} />
      <h3>Z Engine</h3>
      <p className="form-note">Local-first coding agent · v{cfg.version ?? "dev"}</p>
      <dl className="about-dl">
        <dt>Config</dt>
        <dd>
          <code>~/.config/z-engine/config.toml</code>
          <span className="form-note"> falls back to ~/.config/harness if missing</span>
        </dd>
        <dt>Project</dt>
        <dd>
          <code>.z-engine/config.toml</code>
        </dd>
        <dt>Sessions</dt>
        <dd>
          <code>Application Support/z-engine/sessions</code>
        </dd>
        <dt>Model</dt>
        <dd>
          <code>{cfg.model}</code>
        </dd>
      </dl>
    </div>
  );
}
