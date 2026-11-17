import path from 'node:path'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

const host = process.env.TAURI_DEV_HOST
const projectRoot = import.meta.dirname

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: { '@': path.resolve(projectRoot, 'src') },
  },
  build: {
    target: ['es2022', 'chrome105', 'safari13'],
    minify: process.env.TAURI_DEBUG ? false : 'oxc',
    sourcemap: Boolean(process.env.TAURI_DEBUG),
    rollupOptions: {
      input: {
        settings: path.resolve(projectRoot, 'index.html'),
        toolbar: path.resolve(projectRoot, 'toolbar.html'),
        action: path.resolve(projectRoot, 'action.html'),
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
})
