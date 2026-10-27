import { render } from "solid-js/web";
import { App } from "./app/App";
import { createCore } from "./ipc/store";
import { createTauriTransport, insideTauri } from "./ipc/tauri";
import "./styles/app.css";

// Inside a2dpwb.exe the Rust backend talks to the core; in a plain browser
// (pnpm dev) a fake core with sample state stands in for it
const transport = insideTauri() ? await createTauriTransport() : (await import("./ipc/mock")).mockFromUrl();

render(() => <App core={createCore(transport)} />, document.getElementById("root")!);
