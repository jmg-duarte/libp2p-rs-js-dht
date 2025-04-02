import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import wasm from "vite-plugin-wasm";

// https://vite.dev/config/
export default defineConfig({
  plugins: [wasm(), react()],
  server: {
    fs: {
      allow: [".."]
    }
  },
  optimizeDeps: {
    exclude: ['@fs/Users/jose.duarte/Documents/lp2p/kad-query/pkg/index_bg.wasm']
  }
})
