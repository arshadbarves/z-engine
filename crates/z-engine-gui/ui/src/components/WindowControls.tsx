import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isWinPlatform } from "../lib/platform";

/** Chrome-style caption buttons for frameless Windows title bars. */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let live = true;
    const win = getCurrentWindow();
    void win.isMaximized().then((v) => {
      if (live) setMaximized(v);
    });
    const unlisten = win.onResized(() => {
      void win.isMaximized().then((v) => {
        if (live) setMaximized(v);
      });
    });
    return () => {
      live = false;
      void unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="win-controls" aria-label="Window">
      <button
        type="button"
        className="win-ctrl"
        title="Minimize"
        aria-label="Minimize"
        onClick={() => void getCurrentWindow().minimize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <rect x="1" y="4.5" width="8" height="1" fill="currentColor" />
        </svg>
      </button>
      <button
        type="button"
        className="win-ctrl"
        title={maximized ? "Restore" : "Maximize"}
        aria-label={maximized ? "Restore" : "Maximize"}
        onClick={() => void getCurrentWindow().toggleMaximize()}
      >
        {maximized ? (
          <svg viewBox="0 0 10 10" aria-hidden="true">
            <path
              d="M3 1h6v6H3V1zm1 1v4h4V2H4zm2 2h4v4H6V4z"
              fill="currentColor"
            />
          </svg>
        ) : (
          <svg viewBox="0 0 10 10" aria-hidden="true">
            <rect
              x="1.5"
              y="1.5"
              width="7"
              height="7"
              fill="none"
              stroke="currentColor"
              strokeWidth="1"
            />
          </svg>
        )}
      </button>
      <button
        type="button"
        className="win-ctrl close"
        title="Close"
        aria-label="Close"
        onClick={() => void getCurrentWindow().close()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path
            d="M1.8 1.5 5 4.7 8.2 1.5 8.5 1.8 5.3 5 8.5 8.2 8.2 8.5 5 5.3 1.8 8.5 1.5 8.2 4.7 5 1.5 1.8Z"
            fill="currentColor"
          />
        </svg>
      </button>
    </div>
  );
}

/** Only render custom controls on Windows (frameless chrome). */
export function WindowControlsMaybe() {
  if (!isWinPlatform()) return null;
  return <WindowControls />;
}
