import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  root: 'apps/web/src',
  base: '/',
  build: {
    outDir: '../../dist/apps/web',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'apps/web/src/index.html'),
      },
    },
  },
  resolve: {
    alias: {
      '@surf/shared': resolve(__dirname, 'libs/shared/src/generated'),
    },
  },
});
