import { defineConfig, searchForWorkspaceRoot } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri expects a fixed port and wants its own Rust errors to stay visible.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
    // The firmware's version.txt and changelog live at the repository root.
    fs: { allow: [searchForWorkspaceRoot(process.cwd()), "../../version.txt", "../../FIRMWARE_CHANGELOG.md"] },
  },
  build: { target: "es2022" },
});
