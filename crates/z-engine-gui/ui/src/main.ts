import { mount } from "svelte";
import App from "./App.svelte";
import { applyPlatformClass } from "./lib/platform";
import "./index.css";
import "./chrome.css";
import "./transcript.css";
import "./verification.css";
import "./interaction.css";
import "./cards.css";
import "./work.css";
import "./agents.css";

applyPlatformClass();

const target = document.getElementById("root");
if (!target) throw new Error("missing #root");

mount(App, { target });
