import { useEffect, useState, useSyncExternalStore } from "react";
import { ChevronLeft, Info, Server, Settings, Shield } from "lucide-react";
import { getConfig, type HarnessConfig } from "../../lib/commands";
import { updateStore } from "../../lib/updateStore";
import { AboutTab } from "./AboutTab";
import { GeneralTab } from "./GeneralTab";
import { McpTab } from "./McpTab";
import { PermissionsTab } from "./PermissionsTab";
import "../../settings.css";

type Tab = "general" | "permissions" | "mcp" | "about";

const TABS: Array<{ id: Tab; label: string; icon: typeof Settings }> = [
  { id: "general", label: "General", icon: Settings },
  { id: "permissions", label: "Permissions", icon: Shield },
  { id: "mcp", label: "MCP", icon: Server },
  { id: "about", label: "About", icon: Info },
];

export function SettingsPage({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("general");
  const [cfg, setCfg] = useState<HarnessConfig | null>(null);
  const update = useSyncExternalStore(updateStore.subscribe, () => updateStore.getSnapshot());
  const current = TABS.find((t) => t.id === tab);

  useEffect(() => {
    getConfig().then(setCfg).catch(console.error);
  }, [tab]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="settings-page" role="dialog" aria-label="Settings">
      <nav className="settings-nav">
        <button type="button" className="settings-back" onClick={onClose}>
          <ChevronLeft size={15} />
          <span>Back</span>
        </button>
        <div className="settings-brand">Settings</div>
        {TABS.map((t) => {
          const Icon = t.icon;
          return (
            <button
              key={t.id}
              type="button"
              className={`settings-nav-item${tab === t.id ? " active" : ""}`}
              onClick={() => setTab(t.id)}
            >
              <Icon size={15} />
              <span>
                {t.label}
                {t.id === "about" && update.info?.available && (
                  <span className="update-dot inline" role="status" aria-label="Update available" />
                )}
              </span>
            </button>
          );
        })}
      </nav>
      <div className="settings-main">
        <header className="settings-head">
          <h2>{current?.label}</h2>
        </header>
        {!cfg && <div className="tab-body">Loading…</div>}
        {cfg && tab === "general" && <GeneralTab key="g" cfg={cfg} />}
        {cfg && tab === "permissions" && <PermissionsTab key="p" />}
        {cfg && tab === "mcp" && <McpTab key="m" />}
        {cfg && tab === "about" && <AboutTab key="a" cfg={cfg} />}
      </div>
    </div>
  );
}
