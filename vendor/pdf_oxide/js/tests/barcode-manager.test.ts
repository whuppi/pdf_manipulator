import * as fs from 'fs';
import * as path from 'path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BarcodeErrorCorrection, BarcodeFormat, BarcodeManager } from '../src/barcode-manager';
import type { PdfDocument } from '../src/pdf-document';

describe('BarcodeManager', () => {
  let mockDocument: PdfDocument;
  let manager: BarcodeManager;
  let tempDir: string;

  beforeEach(() => {
    mockDocument = {
      filePath: 'test.pdf',
      pageCount: 10,
    } as PdfDocument;
    manager = new BarcodeManager(mockDocument);

    tempDir = path.join(__dirname, '../.tmp', Date.now().toString());
    if (!fs.existsSync(tempDir)) {
      fs.mkdirSync(tempDir, { recursive: true });
    }
  });

  afterEach(() => {
    if (fs.existsSync(tempDir)) {
      fs.rmSync(tempDir, { recursive: true });
    }
  });

  describe('Initialization', () => {
    it('should create manager successfully', () => {
      expect(manager).toBeDefined();
    });

    it('should reject null document', () => {
      expect(() => new BarcodeManager(null as any)).toThrow();
    });
  });

  describe('BarcodeFormat enum', () => {
    it('should have all formats', () => {
      expect(BarcodeFormat.QR_CODE).toBe('qr_code');
      expect(BarcodeFormat.CODE_128).toBe('code_128');
      expect(BarcodeFormat.CODE_39).toBe('code_39');
      expect(BarcodeFormat.EAN_13).toBe('ean_13');
      expect(BarcodeFormat.UPC_A).toBe('upc_a');
      expect(BarcodeFormat.PDF_417).toBe('pdf_417');
      expect(BarcodeFormat.DATA_MATRIX).toBe('data_matrix');
      expect(BarcodeFormat.AZTEC).toBe('aztec');
    });
  });

  describe('BarcodeErrorCorrection enum', () => {
    it('should have all levels', () => {
      expect(BarcodeErrorCorrection.LOW).toBe('low');
      expect(BarcodeErrorCorrection.MEDIUM).toBe('medium');
      expect(BarcodeErrorCorrection.HIGH).toBe('high');
      expect(BarcodeErrorCorrection.VERY_HIGH).toBe('very_high');
    });
  });

  describe('Generate Barcode', () => {
    it('should generate barcode with default config', async () => {
      const result = await manager.generateBarcode('test_data');
      expect(result).toBeInstanceOf(Buffer);
    });

    it('should generate barcode with custom config', async () => {
      const result = await manager.generateBarcode('test', {
        format: BarcodeFormat.CODE_128,
        width: 400,
      });
      expect(result).toBeInstanceOf(Buffer);
    });

    it('should reject empty content', async () => {
      await expect(manager.generateBarcode('')).rejects.toThrow();
    });

    it('should cache barcodes', async () => {
      const result1 = await manager.generateBarcode('test');
      const result2 = await manager.generateBarcode('test');
      expect(result1).toBe(result2);
    });
  });

  describe('Save Barcode File', () => {
    it('should save barcode to file', async () => {
      const outputPath = path.join(tempDir, 'barcode.png');
      const result = await manager.generateBarcodeFile('test_data', outputPath);

      expect(fs.existsSync(result)).toBe(true);
      expect(result).toMatch(/barcode\.png$/);
    });

    it('should create parent directories', async () => {
      const outputPath = path.join(tempDir, 'subdir', 'barcode.png');
      await manager.generateBarcodeFile('test_data', outputPath);

      expect(fs.existsSync(path.dirname(outputPath))).toBe(true);
    });
  });

  describe('Batch Operations', () => {
    it('should generate batch barcodes', async () => {
      const contents = ['CODE-001', 'CODE-002', 'CODE-003'];
      const result = await manager.batchGenerateBarcodes(contents);

      expect(result.size).toBe(3);
      for (const content of contents) {
        expect(result.has(content)).toBe(true);
        expect(result.get(content)).toBeInstanceOf(Buffer);
      }
    });

    it('should reject empty contents', async () => {
      await expect(manager.batchGenerateBarcodes([])).rejects.toThrow();
    });
  });

  describe('Size Estimation', () => {
    it('should estimate QR code size', () => {
      const size = manager.getBarcodeSizeEstimate({
        format: BarcodeFormat.QR_CODE,
        width: 256,
        height: 256,
      } as any);
      expect(size).toBeGreaterThan(0);
      expect(size).toBe(Math.floor(256 * 256 * 3 * 0.25));
    });

    it('should estimate linear barcode size', () => {
      const size = manager.getBarcodeSizeEstimate({
        format: BarcodeFormat.CODE_128,
        width: 300,
        height: 75,
      } as any);
      expect(size).toBeGreaterThan(0);
      expect(size).toBe(Math.floor(300 * 75 * 3 * 0.2));
    });

    it('should estimate 2D barcode size', () => {
      const size = manager.getBarcodeSizeEstimate({
        format: BarcodeFormat.DATA_MATRIX,
        width: 256,
        height: 256,
      } as any);
      expect(size).toBeGreaterThan(0);
      expect(size).toBe(Math.floor(256 * 256 * 3 * 0.3));
    });
  });

  describe('Barcode Embedding', () => {
    it('should embed barcode in page', async () => {
      const result = await manager.embedBarcode(0, 'test_barcode');
      expect(result).toBe(true);
    });

    it('should reject invalid page index', async () => {
      await expect(manager.embedBarcode(10, 'test')).rejects.toThrow();
    });

    it('should reject negative position', async () => {
      await expect(manager.embedBarcode(0, 'test', undefined, -10.0)).rejects.toThrow();
    });
  });

  describe('Barcode Recognition', () => {
    it('should recognize barcodes in page', async () => {
      const result = await manager.recognizeBarcodes(0);
      expect(Array.isArray(result)).toBe(true);
    });

    it('should reject invalid page index', async () => {
      await expect(manager.recognizeBarcodes(10)).rejects.toThrow();
    });

    it('should recognize all barcodes', async () => {
      const result = await manager.recognizeAllBarcodes();
      expect(result instanceof Map).toBe(true);
    });
  });

  describe('Statistics', () => {
    it('should get statistics', () => {
      const stats = manager.getBarcodeStatistics();
      expect(stats).toHaveProperty('total_cached_barcodes');
      expect(stats).toHaveProperty('total_cache_size_bytes');
      expect(stats).toHaveProperty('formats_used');
      expect(stats).toHaveProperty('page_count');
    });

    it('should track cached barcodes', async () => {
      await manager.generateBarcode('test1');
      await manager.generateBarcode('test2');

      const stats = manager.getBarcodeStatistics();
      expect(stats.total_cached_barcodes).toBe(2);
    });
  });

  describe('Cache Management', () => {
    it('should clear barcode cache', async () => {
      await manager.generateBarcode('test');

      let stats = manager.getBarcodeStatistics();
      expect(stats.total_cached_barcodes as number).toBeGreaterThan(0);

      manager.clearBarcodeCache();

      stats = manager.getBarcodeStatistics();
      expect(stats.total_cached_barcodes as number).toBe(0);
    });

    it('should clear cache', async () => {
      await manager.generateBarcode('test');
      manager.clearCache();

      const stats = manager.getBarcodeStatistics();
      expect(stats.total_cached_barcodes as number).toBe(0);
    });
  });
});
