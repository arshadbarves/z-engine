import { configStore } from "./configStore";
import {
  usageStore,
  modelStore,
  modeStore,
  sessionStore,
  pushNotice,
  submitLocal,
  setBusy,
} from "./events";
import { compact, notes, readSlashCommand, submit } from "./commands";
import { getCustomCommands } from "./slash";
import { estimateCost, fmtCost } from "./util";
import { modLabel } from "./platform";

export function dispatchSlashCommand(name: string, input: string): void {
  const custom = getCustomCommands().find((c) => c.name === name);
  if (custom) {
    const args = input.replace(/^\/\S*\s*/, "").trim();
    void (async () => {
      try {
        const template = await readSlashCommand(name);
        const prompt = template.replaceAll("$ARGUMENTS", args).replace(/\s+$/, "");
        submitLocal(prompt);
        setBusy(true);
        await submit(prompt);
      } catch (e) {
        console.error(e);
        setBusy(false);
        pushNotice(`/${name}: ${String(e)}`);
      }
    })();
    return;
  }

  switch (name) {
    case "compact":
      void compact();
      break;
    case "notes":
      void notes();
      break;
    case "help":
      pushNotice(
        "commands: /help /compact /notes /cost /status\n" +
          `keys: Enter send · Esc abort · ⇧⏎ newline · ! shell · @ files · ${modLabel()}K palette`,
      );
      break;
    case "cost": {
      const u = usageStore.getSnapshot();
      const cfg = configStore.getSnapshot();
      const cost = estimateCost(
        cfg?.pricing ?? null,
        u.promptTokens,
        u.completionTokens,
      );
      const total = u.promptTokens + u.completionTokens;
      pushNotice(
        `tokens this session: prompt=${u.promptTokens} completion=${u.completionTokens} total=${total}` +
          (cost != null ? ` · est. ${fmtCost(cost)}` : " · $–"),
      );
      break;
    }
    case "status": {
      const u = usageStore.getSnapshot();
      const cfg = configStore.getSnapshot();
      pushNotice(
        `model=${cfg?.model || modelStore.getSnapshot()} · mode=${modeStore.getSnapshot()} · session=${
          sessionStore.getSnapshot() || "(new)"
        } · tokens ${u.promptTokens + u.completionTokens}/${u.maxTokens}`,
      );
      break;
    }
  }
}
