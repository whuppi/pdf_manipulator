/**
 * Comprehensive test suite for RenderingManager in Node.js/TypeScript
 *
 * Tests validate:
 * - RenderOptions configuration and validation
 * - Single page rendering (file and bytes)
 * - Batch page rendering
 * - Error handling and edge cases
 * - Cross-language API consistency
 */

import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import { PdfDocument, RenderingManager, RenderOptions } from '../lib';

/**
 * Temporary directory helper for test cleanup
 */
class TempDirectory {
  readonly path: string;

  constructor() {
    this.path = fs.mkdtempSync(path.join(os.tmpdir(), 'pdf_oxide_'));
  }

  cleanup(): void {
    try {
      if (fs.existsSync(this.path)) {
        fs.rmSync(this.path, { recursive: true, force: true });
      }
    } catch (error) {
      // Ignore cleanup errors
    }
  }
}

describe('RenderOptions', () => {
  test('defaultOptions has correct values', () => {
    const opts = new RenderOptions();
    expect(opts.dpi).toBe(150);
    expect(opts.format).toBe('png');
    expect(opts.quality).toBe(95);
    expect(opts.maxWidth).toBeUndefined();
    expect(opts.maxHeight).toBeUndefined();
  });

  test.each([1, 72, 150, 300, 600])(
    'validDpi %p accepted',
    (dpi) => {
      const opts = new RenderOptions({ dpi });
      expect(opts.dpi).toBe(dpi);
    }
  );

  test.each([0, -1, 601, 1000])(
    'invalidDpi %p throws error',
    (dpi) => {
      expect(() => new RenderOptions({ dpi })).toThrow();
    }
  );

  test.each(['png', 'jpeg'])(
    'validFormat %p accepted',
    (format) => {
      const opts = new RenderOptions({ format });
      expect(opts.format).toBe(format);
    }
  );

  test.each(['jpg', 'bmp', 'gif', ''])(
    'invalidFormat %p throws error',
    (format) => {
      expect(() => new RenderOptions({ format })).toThrow();
    }
  );

  test.each([1, 50, 95, 100])(
    'validQuality %p accepted',
    (quality) => {
      const opts = new RenderOptions({ quality });
      expect(opts.quality).toBe(quality);
    }
  );

  test.each([0, -1, 101, 150])(
    'invalidQuality %p throws error',
    (quality) => {
      expect(() => new RenderOptions({ quality })).toThrow();
    }
  );

  test('dimensionsValidation accepts valid values', () => {
    const opts = new RenderOptions({ maxWidth: 1920, maxHeight: 1080 });
    expect(opts.maxWidth).toBe(1920);
    expect(opts.maxHeight).toBe(1080);
  });

  test('dimensionsValidation allows null', () => {
    const opts = new RenderOptions({ maxWidth: undefined, maxHeight: undefined });
    expect(opts.maxWidth).toBeUndefined();
    expect(opts.maxHeight).toBeUndefined();
  });

  test.each([0, -1])(
    'invalidDimension %p throws error',
    (dimension) => {
      expect(() => new RenderOptions({ maxWidth: dimension })).toThrow();
    }
  );

  test('draftPreset has correct values', () => {
    const opts = RenderOptions.draft();
    expect(opts.dpi).toBe(72);
    expect(opts.format).toBe('jpeg');
    expect(opts.quality).toBe(70);
  });

  test('normalPreset has correct values', () => {
    const opts = RenderOptions.normal();
    expect(opts.dpi).toBe(150);
    expect(opts.format).toBe('jpeg');
    expect(opts.quality).toBe(85);
  });

  test('highPreset has correct values', () => {
    const opts = RenderOptions.high();
    expect(opts.dpi).toBe(300);
    expect(opts.format).toBe('png');
    expect(opts.quality).toBe(95);
  });

  test('defaultPreset has correct values', () => {
    const opts = RenderOptions.default();
    expect(opts.dpi).toBe(150);
    expect(opts.format).toBe('png');
    expect(opts.quality).toBe(95);
  });

  test('fluentBuilder chains correctly', () => {
    const opts = new RenderOptions()
      .withDpi(300)
      .withFormat('png')
      .withQuality(95);
    expect(opts.dpi).toBe(300);
    expect(opts.format).toBe('png');
    expect(opts.quality).toBe(95);
  });

  test('fluentBuilder returns new instance', () => {
    const opts1 = new RenderOptions({ dpi: 150 });
    const opts2 = opts1.withDpi(300);
    expect(opts1.dpi).toBe(150);
    expect(opts2.dpi).toBe(300);
    expect(opts1).not.toBe(opts2);
  });
});

describe('RenderPageToFile', () => {
  let document: PdfDocument;
  let manager: RenderingManager;
  const samplePdfPath = 'tests/data/simple.pdf';

  beforeAll(async () => {
    if (!fs.existsSync(samplePdfPath)) {
      throw new Error(`Sample PDF not found: ${samplePdfPath}`);
    }
    document = await PdfDocument.open(samplePdfPath);
    manager = new RenderingManager(document);
  });

  afterAll(() => {
    document.close();
  });

  test('renderPngFile succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const outputPath = path.join(tempDir.path, 'page.png');
      const resultPath = await manager.renderPageToFile(
        0,
        outputPath,
        RenderOptions.high()
      );
      expect(fs.existsSync(resultPath)).toBe(true);
      expect(fs.statSync(resultPath).size).toBeGreaterThan(0);
      expect(resultPath).toMatch(/\.png$/);
    } finally {
      tempDir.cleanup();
    }
  });

  test('renderJpegFile succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const outputPath = path.join(tempDir.path, 'page.jpg');
      const resultPath = await manager.renderPageToFile(
        0,
        outputPath,
        RenderOptions.normal()
      );
      expect(fs.existsSync(resultPath)).toBe(true);
      expect(fs.statSync(resultPath).size).toBeGreaterThan(0);
      expect(resultPath).toMatch(/\.jpg$/);
    } finally {
      tempDir.cleanup();
    }
  });

  test('createParentDirectories succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const nestedPath = path.join(tempDir.path, 'output', 'images', 'page.png');
      const resultPath = await manager.renderPageToFile(
        0,
        nestedPath,
        RenderOptions.default()
      );
      expect(fs.existsSync(resultPath)).toBe(true);
      expect(fs.existsSync(path.dirname(resultPath))).toBe(true);
    } finally {
      tempDir.cleanup();
    }
  });

  test('overwriteExistingFile succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const outputPath = path.join(tempDir.path, 'page.png');
      const path1 = await manager.renderPageToFile(
        0,
        outputPath,
        RenderOptions.default()
      );
      const size1 = fs.statSync(path1).size;

      const path2 = await manager.renderPageToFile(
        0,
        outputPath,
        RenderOptions.high()
      );
      const size2 = fs.statSync(path2).size;

      expect(path1).toBe(path2);
      expect(size2).toBeGreaterThan(0);
    } finally {
      tempDir.cleanup();
    }
  });

  test('absolutePathReturned', async () => {
    const tempDir = new TempDirectory();
    try {
      const outputPath = path.join(tempDir.path, 'page.png');
      const resultPath = await manager.renderPageToFile(
        0,
        outputPath,
        RenderOptions.default()
      );
      expect(path.isAbsolute(resultPath)).toBe(true);
    } finally {
      tempDir.cleanup();
    }
  });

  test('invalidPageIndex throws error', async () => {
    const tempDir = new TempDirectory();
    try {
      const outputPath = path.join(tempDir.path, 'page.png');
      await expect(
        manager.renderPageToFile(9999, outputPath, RenderOptions.default())
      ).rejects.toThrow();
    } finally {
      tempDir.cleanup();
    }
  });

  test('invalidOutputPath throws error', async () => {
    await expect(
      manager.renderPageToFile(0, null as any, RenderOptions.default())
    ).rejects.toThrow();
  });
});

describe('RenderPageToBytes', () => {
  let document: PdfDocument;
  let manager: RenderingManager;
  const samplePdfPath = 'tests/data/simple.pdf';

  beforeAll(async () => {
    if (!fs.existsSync(samplePdfPath)) {
      throw new Error(`Sample PDF not found: ${samplePdfPath}`);
    }
    document = await PdfDocument.open(samplePdfPath);
    manager = new RenderingManager(document);
  });

  afterAll(() => {
    document.close();
  });

  test('renderPngBytes succeeds', async () => {
    const imageData = await manager.renderPageToBytes(0, RenderOptions.high());
    expect(imageData).toBeTruthy();
    expect(imageData.length).toBeGreaterThan(0);
    // PNG magic bytes: 89 50 4E 47
    expect(imageData[0]).toBe(0x89);
    expect(imageData[1]).toBe(0x50);
    expect(imageData[2]).toBe(0x4e);
    expect(imageData[3]).toBe(0x47);
  });

  test('renderJpegBytes succeeds', async () => {
    const imageData = await manager.renderPageToBytes(0, RenderOptions.normal());
    expect(imageData).toBeTruthy();
    expect(imageData.length).toBeGreaterThan(0);
    // JPEG magic bytes: FF D8 FF
    expect(imageData[0]).toBe(0xff);
    expect(imageData[1]).toBe(0xd8);
    expect(imageData[2]).toBe(0xff);
  });

  test('bytesCanBeSaved succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const outputFile = path.join(tempDir.path, 'page.png');
      const imageData = await manager.renderPageToBytes(0, RenderOptions.high());
      fs.writeFileSync(outputFile, imageData);
      const readData = fs.readFileSync(outputFile);
      expect(readData).toEqual(imageData);
    } finally {
      tempDir.cleanup();
    }
  });

  test('consistentRenders produce similar results', async () => {
    const data1 = await manager.renderPageToBytes(0, RenderOptions.default());
    const data2 = await manager.renderPageToBytes(0, RenderOptions.default());
    expect(data1.length).toBeGreaterThan(0);
    expect(data2.length).toBeGreaterThan(0);
  });

  test('invalidPageIndex throws error', async () => {
    await expect(
      manager.renderPageToBytes(9999, RenderOptions.default())
    ).rejects.toThrow();
  });
});

describe('RenderPagesRange', () => {
  let document: PdfDocument;
  let manager: RenderingManager;
  const samplePdfPath = 'tests/data/multipage.pdf';

  beforeAll(async () => {
    if (!fs.existsSync(samplePdfPath)) {
      throw new Error(`Sample PDF not found: ${samplePdfPath}`);
    }
    document = await PdfDocument.open(samplePdfPath);
    manager = new RenderingManager(document);
  });

  afterAll(() => {
    document.close();
  });

  test('renderPageRange succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const files = await manager.renderPagesRange(
        0,
        9,
        tempDir.path,
        undefined,
        RenderOptions.default()
      );
      expect(files).toHaveLength(10);
      files.forEach((filePath) => {
        expect(fs.existsSync(filePath)).toBe(true);
        expect(fs.statSync(filePath).size).toBeGreaterThan(0);
      });
    } finally {
      tempDir.cleanup();
    }
  });

  test('defaultFilenamePattern succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const files = await manager.renderPagesRange(0, 2, tempDir.path);
      expect(files).toHaveLength(3);
      const basenames = files.map((f) => path.basename(f));
      expect(basenames).toContain('page_0000.png');
      expect(basenames).toContain('page_0001.png');
      expect(basenames).toContain('page_0002.png');
    } finally {
      tempDir.cleanup();
    }
  });

  test('customFilenamePattern succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const files = await manager.renderPagesRange(
        0,
        2,
        tempDir.path,
        'page_%03d.jpg',
        RenderOptions.normal()
      );
      expect(files).toHaveLength(3);
      const basenames = files.map((f) => path.basename(f));
      expect(basenames).toContain('page_000.jpg');
      expect(basenames).toContain('page_001.jpg');
      expect(basenames).toContain('page_002.jpg');
    } finally {
      tempDir.cleanup();
    }
  });

  test('createOutputDirectory succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const outputDir = path.join(tempDir.path, 'new_output', 'nested');
      const files = await manager.renderPagesRange(0, 1, outputDir);
      expect(fs.existsSync(outputDir)).toBe(true);
      expect(files).toHaveLength(2);
    } finally {
      tempDir.cleanup();
    }
  });

  test('invalidPageRange startGreaterThanEnd throws error', async () => {
    const tempDir = new TempDirectory();
    try {
      await expect(
        manager.renderPagesRange(5, 2, tempDir.path)
      ).rejects.toThrow();
    } finally {
      tempDir.cleanup();
    }
  });

  test('invalidPageRange negativeStart throws error', async () => {
    const tempDir = new TempDirectory();
    try {
      await expect(
        manager.renderPagesRange(-1, 5, tempDir.path)
      ).rejects.toThrow();
    } finally {
      tempDir.cleanup();
    }
  });

  test('singlePageRange succeeds', async () => {
    const tempDir = new TempDirectory();
    try {
      const files = await manager.renderPagesRange(0, 0, tempDir.path);
      expect(files).toHaveLength(1);
      expect(fs.existsSync(files[0])).toBe(true);
    } finally {
      tempDir.cleanup();
    }
  });
});

describe('Caching', () => {
  let document: PdfDocument;
  let manager: RenderingManager;
  const samplePdfPath = 'tests/data/simple.pdf';

  beforeAll(async () => {
    if (!fs.existsSync(samplePdfPath)) {
      throw new Error(`Sample PDF not found: ${samplePdfPath}`);
    }
    document = await PdfDocument.open(samplePdfPath);
    manager = new RenderingManager(document);
  });

  afterAll(() => {
    document.close();
  });

  test('cacheHit returns same bytes on repeat render', async () => {
    const opts = RenderOptions.default();
    const data1 = await manager.renderPageToBytes(0, opts);
    const data2 = await manager.renderPageToBytes(0, opts);
    expect(data1).toEqual(data2);
  });

  test('cacheMiss different options produce different output', async () => {
    const dataDefault = await manager.renderPageToBytes(0, RenderOptions.default());
    const dataHigh = await manager.renderPageToBytes(0, RenderOptions.high());
    expect(dataDefault.length).toBeGreaterThan(0);
    expect(dataHigh.length).toBeGreaterThan(0);
    expect(dataDefault).not.toEqual(dataHigh);
  });
});

describe('CrossLanguageConsistency', () => {
  test('apiMethodsExist', () => {
    expect(typeof RenderingManager.prototype.renderPageToFile).toBe('function');
    expect(typeof RenderingManager.prototype.renderPageToBytes).toBe('function');
    expect(typeof RenderingManager.prototype.renderPagesRange).toBe('function');
  });

  test('renderOptionsParametersExist', () => {
    const opts = new RenderOptions({
      dpi: 150,
      format: 'png',
      quality: 95,
      maxWidth: 1920,
      maxHeight: 1080,
    });
    expect(opts.dpi).toBe(150);
    expect(opts.format).toBe('png');
    expect(opts.quality).toBe(95);
    expect(opts.maxWidth).toBe(1920);
    expect(opts.maxHeight).toBe(1080);
  });

  test('qualityPresetsAvailable', () => {
    expect(RenderOptions.draft()).toBeTruthy();
    expect(RenderOptions.normal()).toBeTruthy();
    expect(RenderOptions.high()).toBeTruthy();
    expect(RenderOptions.default()).toBeTruthy();
  });
});
