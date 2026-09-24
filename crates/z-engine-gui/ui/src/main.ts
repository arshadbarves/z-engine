import { mount } from "svelte";
import App from "./App.svelte";
import { appInfo } from "./lib/commands";
import { applyPlatformClass, applyWindowMaterial } from "./lib/platform";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/motion.css";
import "./styles/materials.css";
import "./index.css";
import "./chrome.css";
import "./transcript.css";
import "./verification.css";
import "./interaction.css";
import "./cards.css";
import "./work.css";
import "./agents.css";
import "./styles/shell.css";
import "./styles/sidebar.css";
import "./styles/companion.css";
import "./styles/title-status.css";
import "./styles/popovers.css";

applyPlatformClass();
appInfo()
  .then((info) => applyWindowMaterial(info.nativeGlass))
  .catch(() => applyWindowMaterial(false));

const target = document.getElementById("root");
if (!target) throw new Error("missing #root");

mount(App, { target });
