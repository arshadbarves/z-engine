import { checkForUpdate, installUpdate, openReleaseUrl, type UpdateInfo } from "./commands";
import { pushToast } from "./events";

type Snapshot = {
  info: UpdateInfo | null;
  checking: boolean;
  installing: boolean;
};

let info: UpdateInfo | null = null;
let checking = false;
let installing = false;
let toastShown = false;
const subs = new Set<() => void>();
let snapshot: Snapshot = { info, checking, installing };

function emit() {
  snapshot = { info, checking, installing };
  for (const l of subs) l();
}

export const updateStore = {
  subscribe(l: () => void) {
    subs.add(l);
    return () => {
      subs.delete(l);
    };
  },
  getSnapshot(): Snapshot {
    return snapshot;
  },
  async check(force = false) {
    if (checking || installing) return;
    checking = true;
    emit();
    try {
      info = await checkForUpdate(force);
      emit();
      if (info.available && !toastShown) {
        toastShown = true;
        pushToast(`Update v${info.latest} available`, "info");
      }
    } catch (e) {
      console.error(e);
    } finally {
      checking = false;
      emit();
    }
  },
  async install() {
    if (checking || installing || !info?.available) return;
    installing = true;
    emit();
    pushToast(`Downloading v${info.latest}…`, "info");
    try {
      await installUpdate();
      // request_restart() ends the process; no return expected.
    } catch (e) {
      console.warn("updater install failed, falling back to browser", e);
      if (info.url) {
        pushToast("Opening release download…", "info");
        await openReleaseUrl(info.url);
      } else {
        pushToast("Could not install update automatically", "warn");
      }
    } finally {
      installing = false;
      emit();
    }
  },
  openRelease() {
    if (info?.url) void openReleaseUrl(info.url);
  },
};
