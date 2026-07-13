#!/usr/bin/env node
/**
 * Post-build script to add .js extensions to ESM imports
 * This is necessary for ES modules that need explicit file extensions
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const libDir = path.join(__dirname, '../lib');

/**
 * Recursively process all .js files in a directory
 */
function processDirectory(dir) {
  const files = fs.readdirSync(dir);

  for (const file of files) {
    const filePath = path.join(dir, file);
    const stat = fs.statSync(filePath);

    if (stat.isDirectory()) {
      processDirectory(filePath);
    } else if (file.endsWith('.js') && !file.endsWith('.cjs')) {
      fixImportsInFile(filePath);
    }
  }
}

/**
 * Fix imports in a single file by adding .js extensions where needed
 */
function fixImportsInFile(filePath) {
  let content = fs.readFileSync(filePath, 'utf-8');
  const originalContent = content;

  // Pattern to match import statements without .js extension
  // This handles: import ... from './path' (but not './path.js')
  content = content.replace(
    /from\s+(['"])(\.[^'"]+)(['"])/g,
    (match, quote1, importPath, quote2) => {
      // Skip if it already ends with .js or .json or is a node module
      if (
        importPath.endsWith('.js') ||
        importPath.endsWith('.json') ||
        importPath.endsWith('.mjs') ||
        importPath.endsWith('.cjs') ||
        !importPath.startsWith('.')
      ) {
        return match;
      }

      // Add .js extension
      return `from ${quote1}${importPath}.js${quote2}`;
    }
  );

  if (content !== originalContent) {
    fs.writeFileSync(filePath, content, 'utf-8');
    console.log(`✓ Fixed imports in ${path.relative(libDir, filePath)}`);
  }
}

console.log('Fixing ESM imports in lib directory...');
processDirectory(libDir);
console.log('✓ ESM import fixes complete!');
