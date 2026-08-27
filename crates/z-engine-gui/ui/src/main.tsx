import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyPlatformClass } from "./lib/platform";
import "./index.css";
import "./chrome.css";
import App from "./App";

applyPlatformClass();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
