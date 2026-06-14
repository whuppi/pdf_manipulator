import * as fs from 'fs';
import * as path from 'path';
import { beforeEach, describe, expect, it } from 'vitest';
import type { PdfDocument } from '../src/pdf-document';
import { type ThumbnailConfig, ThumbnailManager, ThumbnailSize } from '../src/thumbnail-manager';

describe('ThumbnailManager', () => {
  let mockDocument: PdfDocument;
  let manager: ThumbnailManager;
  let tempDir: string;

  beforeEach(() => {
    mockDocument = {
      filePath: 'test.pdf',
      pageCount: 10,
    } as PdfDocument;
    manager = new ThumbnailManager(mockDocument);

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
      expect(() => new ThumbnailManager(null as any)).toThrow();
    });
  });

  describe('ThumbnailSize enum', () => {
    it('should have correct values', () => {
      expect(ThumbnailSize.SMALL).toBe(96);
      expect(ThumbnailSize.MEDIUM).toBe(160);
      expect(ThumbnailSize.LARGE).toBe(256);
      expect(ThumbnailSize.EXTRA_LARGE).toBe(512);
    });
  });

  describe('Generate Thumbnail', () => {
    it('should generate thumbnail with default config', async () => {
      const result = await manager.generateThumbnail(0);
      expect(result).toBeInstanceOf(Buffer);
    });

    it('should generate thumbnail with custom config', async () => {
      const config: ThumbnailConfig = {
        width: 512,
        height: 512,
        format: 'jpeg',
        quality: 90,
        preserveAspectRatio: false,
        backgroundColor: 'black',
      };
      const result = await manager.generateThumbnail(0, config);
      expect(result).toBeInstanceOf(Buffer);
    });

    it('should reject invalid page index', async () => {
      await expect(manager.generateThumbnail(10)).rejects.toThrow();
    });

    it('should reject negative page index', async () => {
      await expect(manager.generateThumbnail(-1)).rejects.toThrow();
    });

    it('should cache thumbnails', async () => {
      const config: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'png',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };
      const result1 = await manager.generateThumbnail(0, config);
      const result2 = await manager.generateThumbnail(0, config);
      expect(result1).toBe(result2);
    });
  });

  describe('Save Thumbnail File', () => {
    it('should save thumbnail to file', async () => {
      const outputPath = path.join(tempDir, 'thumbnail.png');
      const result = await manager.generateThumbnailFile(0, outputPath);

      expect(fs.existsSync(result)).toBe(true);
      expect(result).toMatch(/thumbnail\.png$/);
    });

    it('should create parent directories', async () => {
      const outputPath = path.join(tempDir, 'subdir', 'thumbnail.png');
      await manager.generateThumbnailFile(0, outputPath);

      expect(fs.existsSync(path.dirname(outputPath))).toBe(true);
    });
  });

  describe('Batch Thumbnail Generation', () => {
    it('should generate batch thumbnails', async () => {
      const result = await manager.generateBatchThumbnails(0, 4);
      expect(result.size).toBe(5);

      for (let i = 0; i < 5; i++) {
        expect(result.has(i)).toBe(true);
        expect(result.get(i)).toBeInstanceOf(Buffer);
      }
    });

    it('should reject invalid page range', async () => {
      await expect(manager.generateBatchThumbnails(0, 10)).rejects.toThrow();
    });

    it('should reject inverted range', async () => {
      await expect(manager.generateBatchThumbnails(5, 2)).rejects.toThrow();
    });

    it('should generate batch files', async () => {
      const result = await manager.generateBatchFiles(0, 2, tempDir);
      expect(result.length).toBe(3);

      for (const filePath of result) {
        expect(fs.existsSync(filePath)).toBe(true);
      }
    });

    it('should generate all thumbnails', async () => {
      const result = await manager.generateAllThumbnails(tempDir);
      expect(result.length).toBe(10);
    });
  });

  describe('Size Estimation', () => {
    it('should estimate PNG size', () => {
      const config: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'png',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };
      const size = manager.getThumbnailSizeEstimate(config);
      expect(size).toBeGreaterThan(0);
    });

    it('should estimate JPEG size', () => {
      const config: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'jpeg',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };
      const size = manager.getThumbnailSizeEstimate(config);
      expect(size).toBeGreaterThan(0);
    });

    it('should estimate JPEG smaller than PNG', () => {
      const configJpeg: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'jpeg',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };
      const configPng: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'png',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };

      const sizeJpeg = manager.getThumbnailSizeEstimate(configJpeg);
      const sizePng = manager.getThumbnailSizeEstimate(configPng);

      expect(sizeJpeg).toBeLessThan(sizePng);
    });
  });

  describe('Size Calculation', () => {
    it('should calculate size for max width only', () => {
      const [width, height] = manager.calculateThumbnailSizeForMax(0, 400);
      expect(width).toBeLessThanOrEqual(400);
      expect(height).toBeGreaterThan(0);
    });

    it('should calculate size for max height only', () => {
      const [width, height] = manager.calculateThumbnailSizeForMax(0, undefined, 300);
      expect(width).toBeGreaterThan(0);
      expect(height).toBeLessThanOrEqual(300);
    });

    it('should return default size when no max specified', () => {
      const [width, height] = manager.calculateThumbnailSizeForMax(0);
      expect(width).toBe(256);
      expect(height).toBe(256);
    });

    it('should reject invalid width', () => {
      expect(() => manager.calculateThumbnailSizeForMax(0, 0)).toThrow();
    });

    it('should reject invalid height', () => {
      expect(() => manager.calculateThumbnailSizeForMax(0, undefined, -1)).toThrow();
    });
  });

  describe('Statistics', () => {
    it('should get statistics', () => {
      const stats = manager.getThumbnailStatistics();
      expect(stats).toHaveProperty('total_cached_thumbnails');
      expect(stats).toHaveProperty('total_cache_size_bytes');
      expect(stats).toHaveProperty('formats_used');
      expect(stats).toHaveProperty('page_count');
    });

    it('should track cached thumbnails', async () => {
      const config: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'png',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };

      await manager.generateThumbnail(0, config);
      await manager.generateThumbnail(1, config);

      const stats = manager.getThumbnailStatistics();
      expect(stats.total_cached_thumbnails).toBe(2);
    });

    it('should track formats used', async () => {
      const configPng: Partial<ThumbnailConfig> = { format: 'png' };
      const configJpeg: Partial<ThumbnailConfig> = { format: 'jpeg' };

      await manager.generateThumbnail(0, configPng);
      await manager.generateThumbnail(1, configJpeg);

      const stats = manager.getThumbnailStatistics();
      expect((stats.formats_used as string[]).includes('png')).toBe(true);
      expect((stats.formats_used as string[]).includes('jpeg')).toBe(true);
    });
  });

  describe('Cache Management', () => {
    it('should clear thumbnail cache', async () => {
      const config: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'png',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };

      await manager.generateThumbnail(0, config);
      expect(manager.getThumbnailStatistics().total_cached_thumbnails as number).toBeGreaterThan(0);

      manager.clearThumbnailCache();
      expect(manager.getThumbnailStatistics().total_cached_thumbnails as number).toBe(0);
    });

    it('should clear all cache', async () => {
      const config: ThumbnailConfig = {
        width: 256,
        height: 256,
        format: 'png',
        quality: 85,
        preserveAspectRatio: true,
        backgroundColor: 'white',
      };

      await manager.generateThumbnail(0, config);
      manager.clearCache();
      expect(manager.getThumbnailStatistics().total_cached_thumbnails as number).toBe(0);
    });
  });
});
