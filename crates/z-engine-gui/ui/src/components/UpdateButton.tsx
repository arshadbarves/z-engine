import { useSyncExternalStore } from "react";
import { Download, LoaderCircle } from "lucide-react";
import { updateStore } from "../lib/updateStore";

/** Green update pill shown in the chat header when a release is available. */
export function UpdateButton() {
  const { info, checking, installing } = useSyncExternalStore(
    updateStore.subscribe,
    () => updateStore.getSnapshot(),
  );

  if (!info?.available) return null;

  const label = installing
    ? "Installing update…"
    : checking
      ? "Checking for updates…"
      : `Update to v${info.latest}`;

  return (
    <button
      type="button"
      className="update-pill"
      title={label}
      aria-label={label}
      disabled={checking || installing}
      onClick={() => void updateStore.install()}
    >
      {installing ? (
        <LoaderCircle size={13} className="spin" />
      ) : (
        <Download size={13} />
      )}
      <span className="update-pill-dot" aria-hidden="true" />
    </button>
  );
}
