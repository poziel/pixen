import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  resolve: { dedupe: ["vue"] },
  clearScreen: false,
  // Native build output and live WebView profiles must not be watched by Vite.
  server: { watch: { ignored: ["**/target/**"] } },
  build: { outDir: "dist" },
});
