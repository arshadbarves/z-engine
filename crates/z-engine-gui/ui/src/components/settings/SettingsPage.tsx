import { useEffect, useState, useSyncExternalStore } from "react";
import { ChevronLeft, Coins, Info, Server, Settings, Shield } from "lucide-react";
import { getConfig, type HarnessConfig } from "../../lib/commands";
import { updateStore } from "../../lib/updateStore";
import { LogoMark } from "../LogoMark";
import { AboutTab } from "./AboutTab";
import { CostTab } from "./CostTab";
import { GeneralTab } from "./GeneralTab";
import { McpTab } from "./McpTab";
import { PermissionsTab } from "./PermissionsTab";
import "../../settings.css";

type Tab = "general" | "permissions" | "mcp" | "cost" | "about";

const TABS: Array<{ id: Tab; label: string; hint: string; icon: typeof Settings }> = [
  { id: "general", label: "General", hint: "Model and context", icon: Settings },
  { id: "permissions", label: "Permissions", hint: "Bash allow rules", icon: Shield },
  { id: "mcp", label: "MCP servers", hint: "External tools", icon: Server },
  { id: "cost", label: "Cost", hint: "Token pricing", icon: Coins },
  { id: "about", label: "About", hint: "Paths and version", icon: Info },
];

export function SettingsPage({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("general");
  const [cfg, setCfg] = useState<HarnessConfig | null>(null);
  const update = useSyncExternalStore(updateStore.subscribe, () => updateStore.getSnapshot());
  const current = TABS.find((t) => t.id === tab);

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
        <button type="button" className="settings-back" onClick={onClose}>
          <ChevronLeft size={15} />
          <span>Back</span>
        </button>
        <div className="settings-brand">
          <LogoMark size={18} />
          <span>Settings</span>
        </div>
        {TABS.map((t) => {
          const Icon = t.icon;
          return (
            <button
              key={t.id}
              className={`settings-nav-item${tab === t.id ? " active" : ""}`}
              onClick={() => setTab(t.id)}
            >
              <Icon size={15} />
              <span className="settings-nav-copy">
                <strong>
                  {t.label}
                  {t.id === "about" && update.info?.available && (
                    <span className="update-dot inline" role="status" aria-label="Update available" />
                  )}
                </strong>
                <em>{t.hint}</em>
              </span>
            </button>
          );
        })}
      </nav>
      <div className="settings-main">
        <header className="settings-head">
          <div>
            <h2>{current?.label}</h2>
            <p className="settings-sub">{current?.hint}</p>
          </div>
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
