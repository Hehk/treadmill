import { defineConfig } from "vite";
import createReScriptPlugin from '@jihchi/vite-plugin-rescript';

export default defineConfig(async () => ({
  plugins: [createReScriptPlugin({
    loader: {
      output: "./lib/es6",
      suffix: ".res.js",
    }
  })],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
