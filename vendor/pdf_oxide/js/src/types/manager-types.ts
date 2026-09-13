/**
 * TypeScript type definitions for all PDF Oxide managers
 *
 * Provides comprehensive type safety for Node.js and TypeScript consumers
 * with full FFI integration and proper error handling.
 */

import { EventEmitter } from 'events';

// ============================================================
// Document Types
// ============================================================

export interface PdfDocumentHandle {
  readonly handle: unknown;
  readonly pageCount: () => Promise<number>;
  readonly extractText: (pageIndex: number) => Promise<string>;
  readonly extractFormFields: () => Promise<FormField[]>;
  readonly getMetadata: () => Promise<Record<string, string>>;
}

// ============================================================
// OCR Types
// ============================================================

export enum OcrLanguage {
  ENGLISH = 'en',
  CHINESE = 'zh',
  JAPANESE = 'ja',
  SPANISH = 'es',
  FRENCH = 'fr',
  PORTUGUESE = 'pt',
  RUSSIAN = 'ru',
  ARABIC = 'ar',
  KOREAN = 'ko',
  VIETNAMESE = 'vi',
}

export interface TextRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface OcrResult {
  readonly pageIndex: number;
  readonly text: string;
  readonly confidence: number;
  readonly regionCount: number;
}

export interface OcrBatchResult {
  readonly startPage: number;
  readonly endPage: number;
  readonly totalPages: number;
  readonly totalSpans: number;
  readonly averageConfidence: number;
  readonly skippedPages: number;
}

// ============================================================
// Compliance Types
// ============================================================

export enum ComplianceType {
  PDF_A_1B = 'pdf_a_1b',
  PDF_A_2B = 'pdf_a_2b',
  PDF_A_3B = 'pdf_a_3b',
  PDF_X_1A = 'pdf_x_1a',
  PDF_X_3 = 'pdf_x_3',
  PDF_UA = 'pdf_ua',
}

export interface ComplianceResult {
  readonly type: string;
  readonly valid: boolean;
  readonly issues: readonly string[];
  readonly severity: 'none' | 'low' | 'medium' | 'high' | 'critical';
}

export interface ComplianceReport {
  readonly timestamp: number;
  readonly results: ComplianceResult[];
  readonly isFullyCompliant: boolean;
}

// ============================================================
// Cache Types
// ============================================================

export interface CacheEntry<T> {
  readonly key: string;
  readonly value: T;
  readonly timestamp: number;
  readonly ttl?: number;
}

export interface CacheStats {
  readonly cacheCount: number;
  readonly totalEntries: number;
  readonly totalSize: number;
  readonly entriesByCache: Map<string, number>;
  readonly hitRate: number;
  readonly missRate: number;
}

// ============================================================
// Form Field Types
// ============================================================

export enum FormFieldType {
  TEXT = 'text',
  CHECKBOX = 'checkbox',
  RADIO = 'radio',
  COMBOBOX = 'combobox',
  LISTBOX = 'listbox',
  SIGNATURE = 'signature',
  DATE = 'date',
}

export interface FormField {
  readonly name: string;
  readonly type: FormFieldType;
  readonly value: string;
  readonly pageIndex: number;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly required?: boolean;
  readonly readonly?: boolean;
}

// ============================================================
// Manager Configuration Types
// ============================================================

export interface ManagerOptions {
  readonly enableCache?: boolean;
  readonly cacheSize?: number;
  readonly enableEvents?: boolean;
  readonly enableLogging?: boolean;
}

export interface ManagerState {
  readonly initialized: boolean;
  readonly hasDocument: boolean;
  readonly operationCount: number;
  readonly errorCount: number;
}

// ============================================================
// Event Types
// ============================================================

export interface ManagerEvent<T = Record<string, unknown>> {
  readonly eventType: string;
  readonly timestamp: number;
  readonly data: T;
  readonly manager: string;
}

export interface ErrorEvent extends ManagerEvent<{ error: Error; operation: string }> {}

// ============================================================
// Batch Processing Types
// ============================================================

export interface BatchOptions {
  readonly pageSize?: number;
  readonly parallelism?: number;
  readonly timeout?: number;
  readonly retryOnError?: boolean;
  readonly maxRetries?: number;
}

export interface BatchProgress {
  readonly processed: number;
  readonly total: number;
  readonly percentage: number;
  readonly status: 'pending' | 'running' | 'completed' | 'failed';
}

export interface BatchResult<T> {
  readonly data: T[];
  readonly progress: BatchProgress;
  readonly startTime: number;
  readonly endTime: number;
  readonly duration: number;
  readonly errors: Error[];
}

// ============================================================
// Metadata Types
// ============================================================

export interface PdfMetadata {
  readonly title?: string;
  readonly author?: string;
  readonly subject?: string;
  readonly keywords?: string;
  readonly creator?: string;
  readonly producer?: string;
  readonly creationDate?: Date;
  readonly modificationDate?: Date;
  readonly pageCount: number;
  readonly format?: string;
}

// ============================================================
// Manager Base Class Type
// ============================================================

export interface IManager extends EventEmitter {
  readonly initialized: boolean;

  destroy(): Promise<void>;
  getState(): ManagerState;
}

export abstract class BaseManager<T extends PdfDocumentHandle = PdfDocumentHandle>
  extends EventEmitter
  implements IManager
{
  private _initialized = false;

  get initialized(): boolean {
    return this._initialized;
  }

  protected set initialized(value: boolean) {
    this._initialized = value;
  }
  protected document: T;
  protected cache: Map<string, unknown> = new Map();
  protected operationCount = 0;
  protected errorCount = 0;

  constructor(
    document: T,
    protected options: ManagerOptions = {}
  ) {
    super();
    this.document = document;
    this.initialized = true;
  }

  abstract destroy(): Promise<void>;

  getState(): ManagerState {
    return {
      initialized: this.initialized,
      hasDocument: !!this.document,
      operationCount: this.operationCount,
      errorCount: this.errorCount,
    };
  }

  protected recordOperation(): void {
    this.operationCount++;
  }

  protected recordError(error: Error): void {
    this.errorCount++;
    this.emit('error', { error, timestamp: Date.now() });
  }

  protected getCached<V>(key: string): V | undefined {
    return this.cache.get(key) as V | undefined;
  }

  protected setCached<V>(key: string, value: V): void {
    if (this.options.enableCache !== false) {
      this.cache.set(key, value);
    }
  }

  protected invalidateCache(pattern?: string): void {
    if (pattern) {
      const keys = Array.from(this.cache.keys()).filter((k) => k.includes(pattern));
      keys.forEach((k) => {
        this.cache.delete(k);
      });
    } else {
      this.cache.clear();
    }
  }
}
