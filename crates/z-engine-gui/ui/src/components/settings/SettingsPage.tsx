import { useEffect, useState, useSyncExternalStore } from "react";
import { ChevronLeft, Info, Server, Sliders, Shield } from "../../lib/icons";
import { getConfig, type HarnessConfig } from "../../lib/commands";
import { updateStore } from "../../lib/updateStore";
import { AboutTab } from "./AboutTab";
import { GeneralTab } from "./GeneralTab";
import { McpTab } from "./McpTab";
import { PermissionsTab } from "./PermissionsTab";
import "../../settings.css";

type Tab = "general" | "permissions" | "mcp" | "about";

const TABS: Array<{ id: Tab; label: string; icon: typeof Sliders }> = [
  { id: "general", label: "General", icon: Sliders },
  { id: "permissions", label: "Permissions", icon: Shield },
  { id: "mcp", label: "MCP Servers", icon: Server },
  { id: "about", label: "About", icon: Info },
];

export function SettingsPage({
  isClosing = false,
  onClose,
}: {
  isClosing?: boolean;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<Tab>("general");
  const [cfg, setCfg] = useState<HarnessConfig | null>(null);
  const update = useSyncExternalStore(updateStore.subscribe, () => updateStore.getSnapshot());

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
    <div className={`settings-page${isClosing ? " is-closing" : ""}`} role="dialog" aria-label="Settings">
      <header className="settings-top-bar">
        <div className="settings-bar-left">
          <button type="button" className="settings-back-btn" onClick={onClose}>
            <ChevronLeft size={15} />
            <span>Back</span>
          </button>
        </div>

        <nav className="settings-tabs-pill" aria-label="Settings tabs">
          {TABS.map((t) => {
            const Icon = t.icon;
            const active = tab === t.id;
            return (
              <button
                key={t.id}
                type="button"
                className={`settings-tab-btn${active ? " active" : ""}`}
                onClick={() => setTab(t.id)}
              >
                <Icon size={13} />
                <span>{t.label}</span>
                {t.id === "about" && update.info?.available && (
                  <span className="update-dot inline" role="status" aria-label="Update available" />
                )}
              </button>
            );
          })}
        </nav>

        <div className="settings-bar-right">
          <span className="settings-esc-hint">
            <kbd>Esc</kbd>
          </span>
        </div>
      </header>

      <main className="settings-content-wrap">
        <div className="settings-content-body">
          {!cfg ? (
            <div className="settings-loading">Loading settings…</div>
          ) : (
            <>
              {tab === "general" && <GeneralTab key="g" cfg={cfg} />}
              {tab === "permissions" && <PermissionsTab key="p" />}
              {tab === "mcp" && <McpTab key="m" />}
              {tab === "about" && <AboutTab key="a" cfg={cfg} />}
            </>
          )}
        </div>
      </main>
    </div>
  );
}
