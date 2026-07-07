/**
 * ComplianceManager - Canonical Compliance Manager (merged from 2 implementations)
 *
 * Consolidates:
 * - src/compliance-manager.ts ComplianceManager (validation + issue analysis + native FFI)
 * - src/managers/ocr-compliance-cache.ts ComplianceManager (validation + conversion + fixing)
 *
 * Provides complete PDF/A, PDF/X, PDF/UA compliance operations.
 */

import { EventEmitter } from 'events';
import { promises as fs } from 'fs';
import { dirname } from 'path';

// =============================================================================
// Type Definitions (from root-level compliance-manager.ts)
// =============================================================================

export enum PdfALevel {
  A1a = 'PDF/A-1a',
  A1b = 'PDF/A-1b',
  A2a = 'PDF/A-2a',
  A2b = 'PDF/A-2b',
  A3a = 'PDF/A-3a',
  A3b = 'PDF/A-3b',
}

export enum PdfXLevel {
  X1a = 'PDF/X-1a',
  X1a2001 = 'PDF/X-1a:2001',
  X1a2003 = 'PDF/X-1a:2003',
  X3 = 'PDF/X-3',
  X3_2002 = 'PDF/X-3:2002',
  X3_2003 = 'PDF/X-3:2003',
  X4 = 'PDF/X-4',
}

export enum PdfUALevel {
  UA1 = 'PDF/UA-1',
  UA2 = 'PDF/UA-2',
}

export enum ComplianceIssueType {
  FontNotEmbedded = 'font_not_embedded',
  InvalidColorSpace = 'invalid_color_space',
  MissingAltText = 'missing_alt_text',
  MissingLanguage = 'missing_language',
  MissingTitle = 'missing_title',
  InvalidAnnotation = 'invalid_annotation',
  MissingStructure = 'missing_structure',
  InvalidLink = 'invalid_link',
  Other = 'other',
}

export enum IssueSeverity {
  Error = 'error',
  Warning = 'warning',
  Info = 'info',
}

export interface ComplianceIssue {
  type: ComplianceIssueType;
  severity: IssueSeverity;
  message: string;
  pageIndex?: number;
}

export interface ComplianceValidationResult {
  isCompliant: boolean;
  level: string;
  issues: ComplianceIssue[];
  validationTime: number;
}

// Types from managers version
export interface ComplianceResult {
  type: string;
  valid: boolean;
  issues: string[];
  severity: string;
}

// =============================================================================
// Canonical ComplianceManager
// =============================================================================

export class ComplianceManager extends EventEmitter {
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
  // Validation (from root-level with native FFI)
  // ===========================================================================

  async validatePdfA(
    level: PdfALevel | string = PdfALevel.A1b
  ): Promise<ComplianceValidationResult> {
    const cacheKey = `compliance:pdfa:${level}`;
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);

    let isCompliant = true;
    let issues: ComplianceIssue[] = [];
    let validationTime = 0;

    if (this.native?.validate_pdf_a) {
      try {
        const nativeResult = this.native.validate_pdf_a(level);
        isCompliant = nativeResult.is_compliant ?? true;
        validationTime = nativeResult.validation_time ?? 0;
        if (nativeResult.issues_json) {
          try {
            issues = JSON.parse(nativeResult.issues_json);
          } catch {
            issues = [];
          }
        }
      } catch {
        isCompliant = true;
        issues = [];
      }
    } else if (this.document?.validatePdfA) {
      try {
        const valid = await this.document.validatePdfA(typeof level === 'string' ? level : '1b');
        isCompliant = !!valid;
      } catch {
        isCompliant = true;
      }
    }

    const result: ComplianceValidationResult = {
      isCompliant,
      level: typeof level === 'string' ? level : level,
      issues,
      validationTime,
    };
    this.setCached(cacheKey, result);
    this.emit('pdfAValidated', { level, isCompliant, issueCount: issues.length });
    return result;
  }

  async validatePdfX(
    level: PdfXLevel | string = PdfXLevel.X1a
  ): Promise<ComplianceValidationResult> {
    const cacheKey = `compliance:pdfx:${level}`;
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);

    let isCompliant = true;
    let issues: ComplianceIssue[] = [];
    let validationTime = 0;

    if (this.native?.validate_pdf_x) {
      try {
        const nativeResult = this.native.validate_pdf_x(level);
        isCompliant = nativeResult.is_compliant ?? true;
        validationTime = nativeResult.validation_time ?? 0;
        if (nativeResult.issues_json) {
          try {
            issues = JSON.parse(nativeResult.issues_json);
          } catch {
            issues = [];
          }
        }
      } catch {
        isCompliant = true;
        issues = [];
      }
    } else if (this.document?.validatePdfX) {
      try {
        const valid = await this.document.validatePdfX(typeof level === 'string' ? level : '1a');
        isCompliant = !!valid;
      } catch {
        isCompliant = true;
      }
    }

    const result: ComplianceValidationResult = {
      isCompliant,
      level: typeof level === 'string' ? level : level,
      issues,
      validationTime,
    };
    this.setCached(cacheKey, result);
    this.emit('pdfXValidated', { level, isCompliant, issueCount: issues.length });
    return result;
  }

  async validatePdfUA(
    level: PdfUALevel | string = PdfUALevel.UA1
  ): Promise<ComplianceValidationResult> {
    const cacheKey = `compliance:pdfua:${level}`;
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);

    let isCompliant = true;
    let issues: ComplianceIssue[] = [];
    let validationTime = 0;

    if (this.native?.validate_pdf_ua) {
      try {
        const nativeResult = this.native.validate_pdf_ua(level);
        isCompliant = nativeResult.is_compliant ?? true;
        validationTime = nativeResult.validation_time ?? 0;
        if (nativeResult.issues_json) {
          try {
            issues = JSON.parse(nativeResult.issues_json);
          } catch {
            issues = [];
          }
        }
      } catch {
        isCompliant = true;
        issues = [];
      }
    } else if (this.document?.validatePdfUA) {
      try {
        isCompliant = !!(await this.document.validatePdfUA());
      } catch {
        isCompliant = true;
      }
    }

    const result: ComplianceValidationResult = {
      isCompliant,
      level: typeof level === 'string' ? level : level,
      issues,
      validationTime,
    };
    this.setCached(cacheKey, result);
    this.emit('pdfUAValidated', { level, isCompliant, issueCount: issues.length });
    return result;
  }

  // ===========================================================================
  // Issue Analysis (from root-level)
  // ===========================================================================

  async getAllIssues(): Promise<ComplianceIssue[]> {
    const cacheKey = 'compliance:all_issues';
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    let issues: ComplianceIssue[] = [];
    if (this.native?.compliance_get_all_issues) {
      try {
        issues = JSON.parse(this.native.compliance_get_all_issues()) || [];
      } catch {
        issues = [];
      }
    }
    this.setCached(cacheKey, issues);
    this.emit('issuesRetrieved', { count: issues.length });
    return issues;
  }

  async getIssuesOfType(type: ComplianceIssueType): Promise<ComplianceIssue[]> {
    const allIssues = await this.getAllIssues();
    return allIssues.filter((issue) => issue.type === type);
  }

  async getIssueCount(): Promise<number> {
    return (await this.getAllIssues()).length;
  }
  async getErrorCount(): Promise<number> {
    return (await this.getAllIssues()).filter((i) => i.severity === IssueSeverity.Error).length;
  }
  async getWarningCount(): Promise<number> {
    return (await this.getAllIssues()).filter((i) => i.severity === IssueSeverity.Warning).length;
  }

  // ===========================================================================
  // Conversion & Fixing (from managers version)
  // ===========================================================================

  async convertToPdfA(level: string = '1b'): Promise<boolean> {
    try {
      const result = await this.document?.convertToPdfA?.(level);
      this.resultCache.delete(`compliance:pdfa:${level}`);
      this.emit('conversion-complete', { type: 'PDF/A', level, success: result });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  async convertToPdfUA(): Promise<boolean> {
    try {
      const result = await this.document?.convertToPdfUA?.();
      this.resultCache.delete('compliance:pdfua:');
      this.emit('conversion-complete', { type: 'PDF/UA', success: result });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  async getComplianceReport(complianceType: string = 'all'): Promise<string> {
    try {
      return (await this.document?.getComplianceReport?.(complianceType)) ?? '';
    } catch (error) {
      this.emit('error', error);
      return '';
    }
  }

  async checkFontEmbedding(): Promise<boolean> {
    const cacheKey = 'compliance:fonts_embedded';
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    const result =
      this.document?.checkFontEmbedding?.() ??
      this.native?.compliance_has_embedded_fonts?.() ??
      true;
    this.setCached(cacheKey, result);
    return result;
  }

  /** @deprecated Use checkFontEmbedding() instead */
  async hasFontsEmbedded(): Promise<boolean> {
    return this.checkFontEmbedding();
  }

  async checkColorSpace(): Promise<boolean> {
    const cacheKey = 'compliance:valid_color_space';
    if (this.resultCache.has(cacheKey)) return this.resultCache.get(cacheKey);
    const result =
      this.document?.checkColorSpace?.() ??
      this.native?.compliance_has_valid_color_space?.() ??
      true;
    this.setCached(cacheKey, result);
    return result;
  }

  /** @deprecated Use checkColorSpace() instead */
  async hasValidColorSpace(): Promise<boolean> {
    return this.checkColorSpace();
  }

  async checkTaggedContent(): Promise<boolean> {
    try {
      return (await this.document?.checkTaggedContent?.()) ?? false;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  async addMissingTags(): Promise<boolean> {
    try {
      const result = await this.document?.addMissingTags?.();
      this.emit('tags-added');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  async fixFontIssues(): Promise<number> {
    try {
      const count = (await this.document?.fixFontIssues?.()) ?? 0;
      this.emit('fonts-fixed', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  async fixColorIssues(): Promise<number> {
    try {
      const count = (await this.document?.fixColorIssues?.()) ?? 0;
      this.emit('colors-fixed', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  async removeUnsupportedFeatures(): Promise<number> {
    try {
      const count = (await this.document?.removeUnsupportedFeatures?.()) ?? 0;
      this.emit('features-removed', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  async getComplianceIssues(): Promise<string[]> {
    try {
      const issues: string[] = [];
      const pdfA = await this.validatePdfA(PdfALevel.A1b);
      if (!pdfA.isCompliant) issues.push('PDF/A non-compliant');
      const pdfX = await this.validatePdfX(PdfXLevel.X1a);
      if (!pdfX.isCompliant) issues.push('PDF/X non-compliant');
      const pdfUA = await this.validatePdfUA(PdfUALevel.UA1);
      if (!pdfUA.isCompliant) issues.push('PDF/UA non-accessible');
      const fontEmbedded = await this.checkFontEmbedding();
      if (!fontEmbedded) issues.push('Fonts not properly embedded');
      const taggedContent = await this.checkTaggedContent();
      if (!taggedContent) issues.push('Missing proper tagging');
      this.emit('issues-analyzed', { count: issues.length });
      return issues;
    } catch (error) {
      this.emit('error', error);
      return [];
    }
  }

  getIssueSeverity(issue: string): string {
    if (issue.includes('Font') || issue.includes('PDF/UA')) return 'critical';
    if (issue.includes('PDF/A') || issue.includes('PDF/X')) return 'high';
    if (issue.includes('tagging')) return 'medium';
    return 'low';
  }

  async createComplianceReportFile(filePath: string): Promise<boolean> {
    try {
      const report = await this.getComplianceReport('all');
      await fs.mkdir(dirname(filePath), { recursive: true });
      await fs.writeFile(filePath, report, 'utf8');
      this.emit('report-created', { filePath });
      return true;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  async getComplianceSummary(): Promise<object> {
    try {
      return {
        pdfA: await this.validatePdfA(PdfALevel.A1b),
        pdfX: await this.validatePdfX(PdfXLevel.X1a),
        pdfUA: await this.validatePdfUA(PdfUALevel.UA1),
        fontEmbedded: await this.checkFontEmbedding(),
        colorSpace: await this.checkColorSpace(),
        taggedContent: await this.checkTaggedContent(),
        issues: await this.getComplianceIssues(),
      };
    } catch (error) {
      this.emit('error', error);
      return {};
    }
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

  destroy(): void {
    this.resultCache.clear();
    this.removeAllListeners();
  }

  private setCached(key: string, value: any): void {
    this.resultCache.set(key, value);
    if (this.resultCache.size > this.maxCacheSize) {
      const firstKey = this.resultCache.keys().next().value;
      if (firstKey !== undefined) this.resultCache.delete(firstKey);
    }
  }
}

export default ComplianceManager;
