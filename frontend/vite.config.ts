import { defineConfig } from "vite";
import { devtools } from "@tanstack/devtools-vite";
import viteReact from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { fileURLToPath, URL } from "node:url";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const SERVER_HOST = "127.0.0.1";
const SERVER_PORT = 3001;
const PROXY_PORT = 3000; // Port where the dev server proxies through

// Read OAuth metadata
const metadataPath = resolve(__dirname, "public/client-metadata.json");
const metadata = JSON.parse(readFileSync(metadataPath, "utf-8"));

console.log("Loaded metadata:", metadata);

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    // injects OAuth-related environment variables
    {
      name: "inject-oauth-env",
      config(_conf, { command }) {
        if (command === "build") {
          // provide your own at build time
          // process.env.VITE_OAUTH_CLIENT_ID = metadata.client_id;
          // process.env.VITE_OAUTH_REDIRECT_URI = metadata.redirect_uris[0];
        } else {
          // In dev mode, use the metadata file values directly
          // (which should be set to your cloudflare tunnel URL)
          process.env.VITE_OAUTH_CLIENT_ID = metadata.client_id;
          process.env.VITE_OAUTH_REDIRECT_URI = metadata.redirect_uris[0];
          process.env.VITE_DEV_SERVER_PORT = "" + PROXY_PORT;

          console.log("Dev mode env vars:", {
            client_id: metadata.client_id,
            redirect_uri: metadata.redirect_uris[0],
          });
        }

        process.env.VITE_CLIENT_URI = metadata.client_uri;
        process.env.VITE_OAUTH_SCOPE = metadata.scope;
      },
    },
    devtools(),
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true,
    }),
    viteReact(),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    host: SERVER_HOST,
    port: SERVER_PORT,
  },
});
