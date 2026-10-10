import { defineConfig } from "vite-plus";

export default defineConfig({
  staged: {
    "!(*.rs)": "node ./scripts/run-oxfmt-tracked.mjs",
    "**/*.rs": "rustfmt --edition 2021",
  },
  server: {
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  test: {
    environment: "jsdom",
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      reportsDirectory: "coverage",
    },
  },
});
