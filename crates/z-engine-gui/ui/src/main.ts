import { mount } from "svelte";
import App from "./App.svelte";
import { applyPlatformClass } from "./lib/platform";
import "./index.css";
import "./chrome.css";

applyPlatformClass();

const target = document.getElementById("root");
if (!target) throw new Error("missing #root");

mount(App, { target });
