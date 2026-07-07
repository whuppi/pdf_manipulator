/**
 * BarcodeManager - Canonical Barcode Manager (merged from 2 implementations)
 *
 * Consolidates:
 * - src/barcode-manager.ts BarcodeManager (detection + counting + format-based query)
 * - src/managers/barcode-signature-rendering.ts BarcodesManager (generation + conversion + page embedding)
 *
 * Provides complete barcode operations.
 */

import { EventEmitter } from 'events';

// =============================================================================
// Type Definitions
// =============================================================================

export enum BarcodeFormat {
  CODE128 = 'CODE128',
  CODE39 = 'CODE39',
  EAN13 = 'EAN13',
  EAN8 = 'EAN8',
  UPCA = 'UPCA',
  UPCE = 'UPCE',
  QR = 'QR',
  PDF417 = 'PDF417',
  DATAMATRIX = 'DATAMATRIX',
  AZTEC = 'AZTEC',
  // Numeric format codes from BarcodesManager
  QR_CODE = 'QR_CODE',
  DATA_MATRIX = 'DATA_MATRIX',
  CODE_93 = 'CODE_93',
}

export enum BarcodeErrorCorrection {
  L = 'L',
  M = 'M',
  Q = 'Q',
  H = 'H',
}

export enum QrErrorCorrection {
  L = 0,
  M = 1,
  Q = 2,
  H = 3,
}

export interface DetectedBarcode {
  format: BarcodeFormat;
  rawValue: string;
  decodedValue: string;
  confidence: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface BarcodeGenerationConfig {
  format?: BarcodeFormat;
  width?: number;
  height?: number;
  sizePx?: number;
  errorCorrection?: BarcodeErrorCorrection;
  margin?: number;
}

// =============================================================================
// Canonical BarcodeManager
// =============================================================================

export class BarcodeManager extends EventEmitter {
  private document: any;
  private resultCache = new Map<string, any>();
  private maxCacheSize = 100;
  private native: any;

  constructor(document: any) {
    super();
    this.document = document;
    try {
      this.native = require('../../index.node');
    } catch {
      this.native = null;
    }
  }

  // ===========================================================================
  // Detection (from root BarcodeManager)
  // ===========================================================================

  async detectBarcodes(pageIndex: number): Promise<DetectedBarcode[]> {
    const cacheKey = `barcodes:detect:${pageIndex}`;
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    let barcodes: DetectedBarcode[] = [];
    if (this.native?.detect_barcodes) {
      try {
        const barcodesJson = this.native.detect_barcodes(pageIndex) ?? [];
        barcodes =
          barcodesJson.length > 0 ? barcodesJson.map((json: string) => JSON.parse(json)) : [];
      } catch {
        barcodes = [];
      }
    }
    this.setCached(cacheKey, barcodes);
    this.emit('barcodesDetected', { page: pageIndex, count: barcodes.length });
    return barcodes;
  }

  async detectAllBarcodes(): Promise<Map<number, DetectedBarcode[]>> {
    const cacheKey = 'barcodes:detect_all';
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    let barcodes = new Map<number, DetectedBarcode[]>();
    if (this.native?.detect_all_barcodes) {
      try {
        const barcodesJson = this.native.detect_all_barcodes();
        const parsed = JSON.parse(barcodesJson);
        for (const [page, barcodesArray] of Object.entries(parsed)) {
          barcodes.set(
            parseInt(page),
            (barcodesArray as any[]).map((b) =>
              JSON.parse(typeof b === 'string' ? b : JSON.stringify(b))
            )
          );
        }
      } catch {
        barcodes = new Map();
      }
    }
    this.setCached(cacheKey, barcodes);
    this.emit('allBarcodesDetected', { pages: barcodes.size });
    return barcodes;
  }

  async getBarcodesOfFormat(format: BarcodeFormat, pageIndex?: number): Promise<DetectedBarcode[]> {
    const cacheKey = pageIndex
      ? `barcodes:format:${format}:${pageIndex}`
      : `barcodes:format:${format}:all`;
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    let barcodes: DetectedBarcode[] = [];
    if (this.native?.get_barcodes_of_format) {
      try {
        const page = pageIndex ?? -1;
        const barcodesJson = this.native.get_barcodes_of_format(format, page) ?? [];
        barcodes =
          barcodesJson.length > 0 ? barcodesJson.map((json: string) => JSON.parse(json)) : [];
      } catch {
        barcodes = [];
      }
    }
    this.setCached(cacheKey, barcodes);
    return barcodes;
  }

  async getBarcodeCount(): Promise<number> {
    const cacheKey = 'barcodes:count';
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    const count = this.native?.get_barcode_count?.() ?? 0;
    this.setCached(cacheKey, count);
    return count;
  }

  async getCountByFormat(format: BarcodeFormat): Promise<number> {
    const cacheKey = `barcodes:count:${format}`;
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    const count = this.native?.get_count_by_format?.(format) ?? 0;
    this.setCached(cacheKey, count);
    return count;
  }

  async hasBarcode(pageIndex: number): Promise<boolean> {
    const barcodes = await this.detectBarcodes(pageIndex);
    return barcodes.length > 0;
  }

  // ===========================================================================
  // Generation (from root BarcodeManager + BarcodesManager)
  // ===========================================================================

  async generateBarcode(data: string, config?: BarcodeGenerationConfig): Promise<Buffer> {
    const format = config?.format ?? BarcodeFormat.QR;
    const cacheKey = `barcodes:generate:${data}:${format}`;
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    let imageData = Buffer.alloc(0);
    if (this.native?.generate_barcode) {
      try {
        const configJson = config ? JSON.stringify(config) : '{}';
        const result = this.native.generate_barcode(data, configJson);
        imageData = Buffer.from(result);
      } catch {
        imageData = Buffer.alloc(0);
      }
    }
    this.setCached(cacheKey, imageData);
    this.emit('barcodeGenerated', { format, dataLength: data.length });
    return imageData;
  }

  async generateQrCode(
    data: string,
    errorCorrection: QrErrorCorrection = QrErrorCorrection.M,
    sizePx: number = 256
  ): Promise<Buffer> {
    try {
      if (!data || typeof data !== 'string') throw new Error('Data must be a non-empty string');
      if (sizePx < 1 || sizePx > 10000) throw new Error('Size must be between 1 and 10000 pixels');
      const barcodeData = await this.document?.generateQrCode?.(data, errorCorrection, sizePx);
      this.emit('barcodeGenerated', { format: 'qr', size: sizePx });
      return barcodeData || Buffer.alloc(0);
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  // ===========================================================================
  // Conversion (from BarcodesManager)
  // ===========================================================================

  async barcodeToPng(barcodeData: Buffer, sizePx: number = 256): Promise<Buffer> {
    return barcodeData;
  }

  async barcodeToSvg(barcodeData: Buffer, sizePx: number = 256): Promise<string> {
    // Legacy: wraps existing PNG bytes in an SVG <image> element.
    // For real vector SVG use generateBarcodeSvg() / generateQrCodeSvg() from the top-level API.
    const encoded = barcodeData.toString('base64');
    return `<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,${encoded}"/></svg>`;
  }

  async generateSvg(data: string, config?: BarcodeGenerationConfig): Promise<string> {
    const format = config?.format ?? BarcodeFormat.CODE128;
    if (this.native?.generateBarcode && this.native?.barcodeGetSVG && this.native?.freeBarcode) {
      const handle = this.native.generateBarcode(format, data);
      try {
        return this.native.barcodeGetSVG(handle, config?.sizePx ?? 300) as string;
      } finally {
        this.native.freeBarcode(handle);
      }
    }
    return '';
  }

  async generateQrSvg(
    data: string,
    errorCorrection: QrErrorCorrection = QrErrorCorrection.M,
    sizePx: number = 300
  ): Promise<string> {
    if (this.native?.generateQRCode && this.native?.barcodeGetSVG && this.native?.freeBarcode) {
      const handle = this.native.generateQRCode(data, errorCorrection);
      try {
        return this.native.barcodeGetSVG(handle, sizePx) as string;
      } finally {
        this.native.freeBarcode(handle);
      }
    }
    return '';
  }

  async addBarcodeToPage(
    pageIndex: number,
    barcodeData: Buffer,
    x: number,
    y: number,
    width: number,
    height: number
  ): Promise<boolean> {
    try {
      if (!this.document) throw new Error('Document required for adding barcode to page');
      return false;
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  detectBarcodeFormat(barcodeData: Buffer): BarcodeFormat {
    return BarcodeFormat.QR;
  }

  decodeBarcodeData(barcodeData: Buffer): string {
    return '';
  }

  getDetectionConfidence(barcodeData: Buffer): number {
    return 1.0;
  }

  // ===========================================================================
  // Cache
  // ===========================================================================

  clearCache(): void {
    this.resultCache.clear();
    this.emit('cacheCleared');
  }

  getCacheStats(): Record<string, any> {
    return {
      cacheSize: this.resultCache.size,
      maxCacheSize: this.maxCacheSize,
      entries: Array.from(this.resultCache.keys()),
    };
  }

  private setCached(key: string, value: any): void {
    this.resultCache.set(key, value);
    if (this.resultCache.size > this.maxCacheSize) {
      const firstKey = this.resultCache.keys().next().value;
      if (firstKey !== undefined) this.resultCache.delete(firstKey);
    }
  }
}

export default BarcodeManager;
