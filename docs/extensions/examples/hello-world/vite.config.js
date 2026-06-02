import { defineConfig } from "vite";

// Muxy serves resources from `dist/` when it exists, so every path in
// package.json's `muxy` block resolves against the build output.
//
// - `tabs/index.html` is a Rollup input, so Vite emits `dist/tabs/index.html`
//   (with its CSS/JS hashed and rewritten) at the same relative path the
//   `muxy` block references.
// - Listing assets (icon + screenshot) live in `public/assets/`. Vite copies
//   everything under `publicDir` verbatim into `dist/`, producing
//   `dist/assets/icon.svg` and `dist/assets/screenshot-1.png` untouched.
export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: true,
    minify: false,
    rollupOptions: {
      input: {
        main: "tabs/index.html",
      },
    },
  },
});
