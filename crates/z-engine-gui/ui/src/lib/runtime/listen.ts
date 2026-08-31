import { listen } from "@tauri-apps/api/event";
import type { EventPayload } from "../types";
import { handleEvent } from "./dispatch";
import { activateSession } from "./session";
import { rt } from "./state";

export async function initEvents() {
  if (rt.eventsInitialized) return;
  rt.eventsInitialized = true;
  await listen<EventPayload>("appEvent", (e) => handleEvent(e.payload));
  await listen<{ ulid: string }>("sessionChanged", (e) => {
    activateSession(String(e.payload.ulid ?? ""));
  });
}
