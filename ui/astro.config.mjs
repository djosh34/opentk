import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "astro/config";

export default defineConfig({
  output: "static",
  site: "https://watdoenzedaar.nl",
  devToolbar: {
    enabled: false,
  },
  vite: {
    plugins: [tailwindcss()],
    server: {
      proxy: {
        "/tweedekamer-resource": {
          target: "https://gegevensmagazijn.tweedekamer.nl",
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/tweedekamer-resource/, ""),
          configure: (proxy) => {
            proxy.on("proxyRes", (proxyRes) => {
              delete proxyRes.headers["content-disposition"];
              delete proxyRes.headers["content-security-policy"];
              delete proxyRes.headers["set-cookie"];
            });
          },
        },
      },
    },
  },
});
