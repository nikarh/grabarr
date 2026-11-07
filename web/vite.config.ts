import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

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
      // The backend checks that a WebSocket's Origin matches its Host, so the Host isn't rewritten.
      '/api': { target: 'http://localhost:8080', ws: true },
    },
  },
})
