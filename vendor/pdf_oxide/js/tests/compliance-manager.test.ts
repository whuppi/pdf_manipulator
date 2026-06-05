import { beforeEach, describe, expect, it } from 'vitest';
import {
  ComplianceIssueType,
  ComplianceManager,
  PdfALevel,
  PdfUALevel,
  PdfXLevel,
} from '../src/compliance-manager';

describe('ComplianceManager', () => {
  let mockDocument: any;
  let manager: ComplianceManager;

  beforeEach(() => {
    mockDocument = { filePath: 'test.pdf', pageCount: 10 };
    manager = new ComplianceManager(mockDocument);
  });

  describe('Initialization', () => {
    it('should create manager successfully', () => {
      expect(manager).toBeDefined();
    });

    it('should reject null document', () => {
      expect(() => new ComplianceManager(null)).toThrow();
    });

    it('should reject undefined document', () => {
      expect(() => new ComplianceManager(undefined)).toThrow();
    });
  });

  describe('Enum Values', () => {
    it('should have all PdfALevel values', () => {
      expect(PdfALevel.PDF_A_1B).toBe('pdf_a_1b');
      expect(PdfALevel.PDF_A_1A).toBe('pdf_a_1a');
      expect(PdfALevel.PDF_A_2B).toBe('pdf_a_2b');
      expect(PdfALevel.PDF_A_2A).toBe('pdf_a_2a');
      expect(PdfALevel.PDF_A_2U).toBe('pdf_a_2u');
      expect(PdfALevel.PDF_A_3B).toBe('pdf_a_3b');
      expect(PdfALevel.PDF_A_3A).toBe('pdf_a_3a');
      expect(PdfALevel.PDF_A_3U).toBe('pdf_a_3u');
    });

    it('should have all PdfXLevel values', () => {
      expect(PdfXLevel.PDF_X_1A_2001).toBe('pdf_x_1a_2001');
      expect(PdfXLevel.PDF_X_3_2002).toBe('pdf_x_3_2002');
      expect(PdfXLevel.PDF_X_3_2003).toBe('pdf_x_3_2003');
      expect(PdfXLevel.PDF_X_4_2008).toBe('pdf_x_4_2008');
    });

    it('should have all PdfUALevel values', () => {
      expect(PdfUALevel.PDF_UA_1).toBe('pdf_ua_1');
      expect(PdfUALevel.PDF_UA_2).toBe('pdf_ua_2');
    });

    it('should have all ComplianceIssueType values', () => {
      expect(ComplianceIssueType.ERROR).toBe('error');
      expect(ComplianceIssueType.WARNING).toBe('warning');
      expect(ComplianceIssueType.INFO).toBe('info');
    });
  });

  describe('Validation Methods', () => {
    it('should validate PDF/A standard', async () => {
      const result = await manager.validatePdfA(PdfALevel.PDF_A_1B);

      expect(result).toBeDefined();
      expect(result.standard).toBe('PDF/A');
      expect(result.level).toBe('pdf_a_1b');
      expect(result.checkedAt).toBeInstanceOf(Date);
    });

    it('should validate PDF/A with different levels', async () => {
      const levels = [PdfALevel.PDF_A_1B, PdfALevel.PDF_A_2B, PdfALevel.PDF_A_3B];

      for (const level of levels) {
        const result = await manager.validatePdfA(level);
        expect(result.standard).toBe('PDF/A');
        expect(result.level).toBe(level);
      }
    });

    it('should validate PDF/X standard', async () => {
      const result = await manager.validatePdfX(PdfXLevel.PDF_X_1A_2001);

      expect(result).toBeDefined();
      expect(result.standard).toBe('PDF/X');
      expect(result.level).toBe('pdf_x_1a_2001');
    });

    it('should validate PDF/X with different levels', async () => {
      const levels = [PdfXLevel.PDF_X_1A_2001, PdfXLevel.PDF_X_3_2002, PdfXLevel.PDF_X_4_2008];

      for (const level of levels) {
        const result = await manager.validatePdfX(level);
        expect(result.standard).toBe('PDF/X');
        expect(result.level).toBe(level);
      }
    });

    it('should validate PDF/UA standard', async () => {
      const result = await manager.validatePdfUA(PdfUALevel.PDF_UA_1);

      expect(result).toBeDefined();
      expect(result.standard).toBe('PDF/UA');
      expect(result.level).toBe('pdf_ua_1');
    });

    it('should validate PDF/UA with both levels', async () => {
      const levels = [PdfUALevel.PDF_UA_1, PdfUALevel.PDF_UA_2];

      for (const level of levels) {
        const result = await manager.validatePdfUA(level);
        expect(result.standard).toBe('PDF/UA');
        expect(result.level).toBe(level);
      }
    });

    it('should validate all standards', async () => {
      const results = await manager.validateAll();

      expect(results.size).toBeGreaterThan(0);
      expect(results.has('PDF/A')).toBe(true);
      expect(results.has('PDF/X')).toBe(true);
      expect(results.has('PDF/UA')).toBe(true);
    });
  });

  describe('Configuration-based Validation', () => {
    it('should validate with custom config', async () => {
      const result = await manager.validate({
        checkPdfA: true,
        pdfALevel: PdfALevel.PDF_A_3U,
      });

      expect(result).toBeDefined();
    });

    it('should validate with multiple standards', async () => {
      const result = await manager.validate({
        checkPdfA: true,
        pdfALevel: PdfALevel.PDF_A_2A,
        checkPdfX: true,
        pdfXLevel: PdfXLevel.PDF_X_4_2008,
        checkPdfUA: true,
        pdfUALevel: PdfUALevel.PDF_UA_2,
      });

      expect(result).toBeDefined();
    });

    it('should support strict mode', async () => {
      const result = await manager.validate({
        checkPdfA: true,
        pdfALevel: PdfALevel.PDF_A_1A,
        strictMode: true,
      });

      expect(result).toBeDefined();
    });

    it('should support auto-fix mode', async () => {
      const result = await manager.fixComplianceIssues({
        checkPdfA: true,
        pdfALevel: PdfALevel.PDF_A_2B,
        autoFix: true,
      });

      expect(result).toBeDefined();
    });

    it('should reject null config', async () => {
      await expect(manager.validate(null as any)).rejects.toThrow();
    });
  });

  describe('Caching', () => {
    it('should cache same config results', async () => {
      const config = {
        checkPdfA: true,
        pdfALevel: PdfALevel.PDF_A_2B,
      };

      const result1 = await manager.validate(config);
      const result2 = await manager.validate(config);

      expect(result1.standard).toBe(result2.standard);
      expect(result1.level).toBe(result2.level);
    });

    it('should clear cache', async () => {
      const config = { checkPdfA: true };
      await manager.validate(config);
      manager.clearCache();

      // Should still be able to validate
      const result = await manager.validate(config);
      expect(result).toBeDefined();
    });
  });

  describe('Statistics', () => {
    it('should get empty statistics', async () => {
      const stats = await manager.getComplianceStatistics();

      expect(stats).toBeDefined();
      expect(stats.has('total_issues')).toBe(true);
      expect(stats.has('total_errors')).toBe(true);
      expect(stats.has('total_warnings')).toBe(true);
      expect(stats.has('total_info')).toBe(true);
    });

    it('should track statistics with validations', async () => {
      await manager.validatePdfA(PdfALevel.PDF_A_1B);
      await manager.validatePdfX(PdfXLevel.PDF_X_3_2002);
      await manager.validatePdfUA(PdfUALevel.PDF_UA_1);

      const stats = await manager.getComplianceStatistics();

      expect(stats.has('standards_checked')).toBe(true);
      expect(stats.has('compliance_rate')).toBe(true);
    });

    it('should calculate compliance rate', async () => {
      await manager.validatePdfA(PdfALevel.PDF_A_2B);
      const stats = await manager.getComplianceStatistics();

      const rate = stats.get('compliance_rate');
      expect(typeof rate).toBe('number');
      expect(rate).toBeGreaterThanOrEqual(0);
      expect(rate).toBeLessThanOrEqual(100);
    });
  });

  describe('Result Metadata', () => {
    it('should include timestamp', async () => {
      const before = new Date();
      const result = await manager.validatePdfA(PdfALevel.PDF_A_1B);
      const after = new Date();

      expect(result.checkedAt).toBeInstanceOf(Date);
      expect(result.checkedAt.getTime()).toBeGreaterThanOrEqual(before.getTime());
      expect(result.checkedAt.getTime()).toBeLessThanOrEqual(after.getTime());
    });

    it('should include processing time', async () => {
      const result = await manager.validatePdfA(PdfALevel.PDF_A_1B);

      expect(typeof result.processingTimeMs).toBe('number');
      expect(result.processingTimeMs).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Concurrent Operations', () => {
    it('should handle concurrent validations', async () => {
      const promises = [];

      for (let i = 0; i < 5; i++) {
        const levelIndex = i % Object.values(PdfALevel).length;
        const level = Object.values(PdfALevel)[levelIndex];
        promises.push(manager.validatePdfA(level as PdfALevel));
      }

      const results = await Promise.all(promises);

      expect(results.length).toBe(5);
      for (const result of results) {
        expect(result).toBeDefined();
      }
    });

    it('should handle concurrent statistics retrieval', async () => {
      await manager.validatePdfA(PdfALevel.PDF_A_1B);

      const promises = [];
      for (let i = 0; i < 3; i++) {
        promises.push(manager.getComplianceStatistics());
      }

      const results = await Promise.all(promises);

      expect(results.length).toBe(3);
      for (const stats of results) {
        expect(stats).toBeDefined();
      }
    });
  });

  describe('Configuration Combinations', () => {
    it('should validate various flag combinations', async () => {
      const combinations = [
        { checkPdfA: true },
        { checkPdfX: true },
        { checkPdfUA: true },
        { checkPdfA: true, checkPdfX: true },
        { checkPdfA: true, checkPdfUA: true },
        { checkPdfX: true, checkPdfUA: true },
      ];

      for (const config of combinations) {
        const result = await manager.validate(config);
        expect(result).toBeDefined();
      }
    });

    it('should validate strict mode and auto-fix combinations', async () => {
      const combinations = [
        { strictMode: false, autoFix: false },
        { strictMode: true, autoFix: false },
        { strictMode: false, autoFix: true },
        { strictMode: true, autoFix: true },
      ];

      for (const config of combinations) {
        const result = await manager.validate({
          checkPdfA: true,
          pdfALevel: PdfALevel.PDF_A_2B,
          ...config,
        });
        expect(result).toBeDefined();
      }
    });
  });
});
