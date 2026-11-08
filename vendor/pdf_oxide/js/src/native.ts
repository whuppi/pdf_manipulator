/**
 * Shared loader for the pdf-oxide N-API addon.
 *
 * Extracted so that modules which can't import from `./index.ts`
 * (due to the index ↔ editor / builder cycle) can still reach the
 * addon through the same prebuild-aware resolver. The published
 * package ships a platform-specific `.node` file under
 * `prebuilds/<triple>/`; loading the addon through this helper is
 * what makes `DocumentEditor` / `DocumentBuilder` work for consumers
 * installed from npm (where `build/Release/` does not exist).
 *
 * In development mode (`NODE_ENV=development` or `NAPI_DEV` set), we
 * fall back to `../build/Release/pdf_oxide.node` — the node-gyp
 * output that in-tree tests run against.
 */

import { createRequire } from 'node:module';
import { arch, platform } from 'node:os';

const require = createRequire(import.meta.url);

// Prebuild paths are relative to the *compiled* `lib/native.js` — at
// runtime the file lives at `js/lib/native.js`, so `../prebuilds/`
// resolves to `js/prebuilds/`.
const PLATFORMS: Record<string, Record<string, string>> = {
  darwin: {
    x64: '../prebuilds/darwin-x64/pdf_oxide.node',
    arm64: '../prebuilds/darwin-arm64/pdf_oxide.node',
  },
  linux: {
    x64: '../prebuilds/linux-x64/pdf_oxide.node',
    arm64: '../prebuilds/linux-arm64/pdf_oxide.node',
  },
  win32: {
    x64: '../prebuilds/win32-x64/pdf_oxide.node',
  },
};

function getPrebuildPath(): string {
  const os = platform();
  const cpu = arch();
  const osPaths = PLATFORMS[os];
  if (!osPaths) {
    throw new Error(`Unsupported platform: ${os}. Supported: ${Object.keys(PLATFORMS).join(', ')}`);
  }
  const prebuildPath = osPaths[cpu];
  if (!prebuildPath) {
    throw new Error(
      `Unsupported architecture: ${cpu} for ${os}. Supported: ${Object.keys(osPaths).join(', ')}`
    );
  }
  return prebuildPath;
}

let cached: any;

export function loadNative(): any {
  if (cached) return cached;
  try {
    cached = require(getPrebuildPath());
    return cached;
  } catch (e) {
    // Dev fallback — in-tree `node-gyp rebuild` output.
    if (process.env.NODE_ENV === 'development' || process.env.NAPI_DEV) {
      try {
        cached = require('../build/Release/pdf_oxide.node');
        return cached;
      } catch {
        /* fall through to rethrow the original error */
      }
    }
    throw new Error(`Failed to load pdf-oxide native addon: ${(e as Error).message}`);
  }
}
