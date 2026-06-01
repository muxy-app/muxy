import { copyFileSync } from "node:fs";
import { defineConfig } from "vite";

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
  plugins: [
    {
      name: "muxy-manifest",
      closeBundle() {
        copyFileSync("package.json", "dist/package.json");
      },
    },
  ],
});
