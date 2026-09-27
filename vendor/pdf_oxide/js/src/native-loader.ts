// Shared loader for the pdf-oxide native N-API module.
//
// The top-level index.ts owns the primary load, but standalone classes like
// Timestamp and TsaClient need access too. Rather than plumb the module
// object through every constructor, both files import `getNative()` here.
//
// The loader resolves the prebuilt .node via the same prebuilds/<triple>
// layout index.ts uses, with fallbacks for development builds.

import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { arch, platform } from 'node:process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const require = createRequire(import.meta.url);

type NativeModule = Record<string, any>;
let cached: NativeModule | null = null;

function prebuildPath(): string {
  const paths: Record<string, Record<string, string>> = {
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
  const plat = paths[platform];
  const rel = plat?.[arch];
  if (!rel) {
    throw new Error(`Unsupported platform: ${platform}/${arch}`);
  }
  return join(__dirname, rel);
}

/**
 * Returns the loaded native module, loading it on first call. Throws if the
 * prebuilt .node is missing and the development fallback also fails.
 */
export function getNative(): NativeModule {
  if (cached) return cached;
  try {
    cached = require(prebuildPath()) as NativeModule;
  } catch (e) {
    if (process.env.NODE_ENV === 'development' || process.env.NAPI_DEV) {
      try {
        cached = require('../build/Release/pdf_oxide.node') as NativeModule;
      } catch {
        throw e;
      }
    } else {
      throw e;
    }
  }
  return cached as NativeModule;
}
