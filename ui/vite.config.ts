import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';
import { resolve } from 'node:path';

export default defineConfig(({ mode }) => ({
  plugins: [svelte()],
  base: './',
  resolve: {
    conditions: ['browser']
  },
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.ts']
  },
  build: {
    outDir: mode === 'gallery' ? 'dist/gallery' : 'dist',
    emptyOutDir: mode !== 'gallery',
    assetsInlineLimit: Number.MAX_SAFE_INTEGER,
    rollupOptions: {
      input: resolve(import.meta.dirname, mode === 'gallery' ? 'gallery.html' : 'index.html'),
      output: {
        inlineDynamicImports: true,
        entryFileNames: 'assets/[name].js',
        assetFileNames: 'assets/[name][extname]'
      }
    }
  }
}));
