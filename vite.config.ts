import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import pkg from "./package.json";

// Two entries on purpose: the minimap overlay webview must stay minimal (no
// Skeleton, no Leaflet), so it is its own HTML entry with a tiny bundle.
export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  define: {
    // Compile-time so the footer needs no IPC (and no permission) to show it.
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  resolve: {
    alias: {
      $lib: fileURLToPath(new URL("./src/lib", import.meta.url)),
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        minimap: fileURLToPath(new URL("./minimap.html", import.meta.url)),
        nightVision: fileURLToPath(new URL("./night-vision.html", import.meta.url)),
      },
    },
  },
  // Tauri dev server conventions.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
})
