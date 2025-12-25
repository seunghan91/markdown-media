import { defineConfig } from 'vite'
import RubyPlugin from 'vite-plugin-ruby'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import path from 'path'

export default defineConfig({
  plugins: [
    RubyPlugin(),
    svelte({
      configFile: './svelte.config.js'
    }),
  ],
  resolve: {
    conditions: ['svelte', 'browser'],
    alias: {
      '$lib': path.resolve('./app/frontend/lib'),
      '$components': path.resolve('./app/frontend/components'),
      '@': path.resolve('./app/frontend'),
    },
  },
})
