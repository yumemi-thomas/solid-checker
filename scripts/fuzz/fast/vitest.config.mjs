// Written next to the generated tests by fast.mjs (a config must sit inside the install
// to resolve its plugin). Same Solid plugin options as the Happy DOM experiment:
// no HMR, no performance tracks, no compile-time diagnostics (the runtime's own are the
// subject), source locations on so owner `loc` is populated.
import { defineConfig } from 'vitest/config';
import solid from '@solidjs/vite-plugin';

const workers = Number(process.env.FUZZ_FAST_WORKERS ?? 4);
export default defineConfig({
  plugins: [solid({ hot: false, performanceTracks: false, diagnostics: false, solid: { sourceNames: { locations: true } } })],
  resolve: { conditions: ['browser', 'development'], dedupe: ['solid-js', '@solidjs/signals', '@solidjs/web'] },
  // A symlinked node_modules (a shared install) resolves outside the work directory.
  server: { fs: { strict: false } },
  ssr: { resolve: { conditions: ['browser', 'development'], externalConditions: ['browser', 'development'] } },
  test: {
    include: ['tests/*.test.ts'],
    environment: 'happy-dom',
    environmentOptions: { happyDOM: { url: 'http://localhost/' } },
    setupFiles: ['@solidjs/diagnostics/vitest', './shims.ts'],
    maxWorkers: workers,
    isolate: true,
    testTimeout: 60000,
    server: { deps: { inline: ['solid-js', '@solidjs/signals', '@solidjs/web', '@solidjs/diagnostics'] } },
  },
});
