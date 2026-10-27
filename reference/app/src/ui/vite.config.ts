import { fileURLToPath } from "node:url";
import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";

// The translations are shared with the core (app/lang at the repository
// root); the dev server may read them from there
const langDir = fileURLToPath(new URL("../../../app/lang", import.meta.url));

export default defineConfig({
  // Hot reload only for the dev server (its runtime does not load under Vitest)
  plugins: [solid({ hot: !process.env.VITEST })],
  // Tauri's CLI prints its own output next to Vite's
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    fs: { allow: [".", langDir] },
  },
  build: {
    // WebView2 is an evergreen Chromium
    target: "chrome120",
    outDir: "dist",
    emptyOutDir: true,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
});
