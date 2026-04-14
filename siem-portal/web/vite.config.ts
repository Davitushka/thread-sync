import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const proxyTarget = (env.VITE_PORTAL_PROXY_TARGET || "http://127.0.0.1:8091").trim();

  return {
    plugins: [react()],
    server: {
      proxy: {
        "/api/v1": {
          target: proxyTarget,
          changeOrigin: true,
        },
        "/health": {
          target: proxyTarget,
          changeOrigin: true,
        },
      },
    },
    build: {
      outDir: "../static",
      emptyOutDir: true,
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes("node_modules/echarts") || id.includes("node_modules/echarts-for-react")) {
              return "vendor-echarts";
            }
            if (id.includes("node_modules/react") || id.includes("node_modules/react-dom") || id.includes("node_modules/react-router-dom")) {
              return "vendor-react";
            }
            if (id.includes("src/components/echarts/")) {
              return "vendor-echarts";
            }
            return undefined;
          },
        },
      },
    },
  };
});
