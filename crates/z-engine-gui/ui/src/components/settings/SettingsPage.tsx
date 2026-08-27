import { useEffect, useState } from "react";
import { Coins, Info, Server, Settings, Shield, X } from "lucide-react";
import { getConfig, type HarnessConfig } from "../../lib/commands";
import { AboutTab } from "./AboutTab";
import { CostTab } from "./CostTab";
import { GeneralTab } from "./GeneralTab";
import { McpTab } from "./McpTab";
import { PermissionsTab } from "./PermissionsTab";
import "../../settings.css";

type Tab = "general" | "permissions" | "mcp" | "cost" | "about";

const TABS: Array<{ id: Tab; label: string; icon: typeof Settings }> = [
  { id: "general", label: "General", icon: Settings },
  { id: "permissions", label: "Permissions", icon: Shield },
  { id: "mcp", label: "MCP servers", icon: Server },
  { id: "cost", label: "Cost", icon: Coins },
  { id: "about", label: "About", icon: Info },
];

export function SettingsPage({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("general");
  const [cfg, setCfg] = useState<HarnessConfig | null>(null);

  useEffect(() => {
    getConfig().then(setCfg).catch(console.error);
  }, []);

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
        <div className="settings-nav-title">Settings</div>
        {TABS.map((t) => {
          const Icon = t.icon;
          return (
            <button
              key={t.id}
              className={`settings-nav-item${tab === t.id ? " active" : ""}`}
              onClick={() => setTab(t.id)}
            >
              <Icon size={14} />
              {t.label}
            </button>
          );
        })}
      </nav>
      <div className="settings-main">
        <header className="settings-head">
          <h2>{TABS.find((t) => t.id === tab)?.label}</h2>
          <button className="icon-btn" onClick={onClose} title="Close">
            <X size={14} />
          </button>
        </header>
        {!cfg && <div className="tab-body">Loading…</div>}
        {cfg && tab === "general" && <GeneralTab key="g" cfg={cfg} />}
        {cfg && tab === "permissions" && <PermissionsTab key="p" />}
        {cfg && tab === "mcp" && <McpTab key="m" />}
        {cfg && tab === "cost" && <CostTab key="c" cfg={cfg} />}
        {cfg && tab === "about" && <AboutTab key="a" cfg={cfg} />}
      </div>
    </div>
  );
}
