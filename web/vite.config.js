import { defineConfig } from "vite";

// SharedArrayBuffer needs a cross-origin isolated page.
const headers = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  base: "./",
  server: { headers },
  preview: { headers },
});
