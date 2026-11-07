import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The UI against a grabarr running elsewhere, instead of the local backend: GRABARR_REMOTE=https://grabarr.example.
const remote = process.env.GRABARR_REMOTE

// https://vite.dev/config/
export default defineConfig({
  // Relative asset URLs, so the app also works under a path prefix behind a reverse proxy.
  base: './',
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // Built into the binary: no source maps, and a warning only for really large chunks.
    sourcemap: false,
    chunkSizeWarningLimit: 1500,
  },
  server: {
    proxy: {
      '/api': remote
        ? {
            // A running instance elsewhere (`mise run dev:remote`). It checks that a WebSocket's Origin matches its
            // Host, so both name it.
            target: remote,
            ws: true,
            changeOrigin: true,
            secure: true,
            configure: (proxy) => {
              proxy.on('proxyReqWs', (req) => {
                req.setHeader('origin', new URL(remote).origin)
              })
            },
          }
        : // The backend checks that a WebSocket's Origin matches its Host, so the Host isn't rewritten.
          { target: 'http://localhost:8080', ws: true },
    },
  },
})
