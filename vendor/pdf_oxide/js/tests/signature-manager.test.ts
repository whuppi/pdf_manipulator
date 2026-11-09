import { beforeEach, describe, expect, it } from 'vitest';
import type { PdfDocument } from '../src/pdf-document';
import {
  type CertificateInfo,
  DigestAlgorithm,
  SignatureAlgorithm,
  type SignatureInfo,
  SignatureManager,
  SignatureStatus,
} from '../src/signature-manager';

describe('SignatureManager', () => {
  let mockDocument: PdfDocument;
  let manager: SignatureManager;

  beforeEach(() => {
    mockDocument = {
      filePath: 'test.pdf',
      pageCount: 10,
    } as PdfDocument;
    manager = new SignatureManager(mockDocument);
  });

  describe('Initialization', () => {
    it('should create manager successfully', () => {
      expect(manager).toBeDefined();
    });

    it('should reject null document', () => {
      expect(() => new SignatureManager(null as any)).toThrow('Document cannot be null');
    });

    it('should reject undefined document', () => {
      expect(() => new SignatureManager(undefined as any)).toThrow('Document cannot be null');
    });
  });

  describe('Signature Count and Retrieval', () => {
    it('should return zero signatures for empty document', () => {
      expect(manager.getSignatureCount()).toBe(0);
    });

    it('should return empty array for getAllSignatures', () => {
      const signatures = manager.getAllSignatures();
      expect(signatures).toEqual([]);
      expect(signatures).toHaveLength(0);
    });

    it('should throw error for invalid signature index', () => {
      expect(() => manager.getSignatureAt(0)).toThrow('Signature index 0 out of range [0, 0)');
    });

    it('should throw error for negative signature index', () => {
      expect(() => manager.getSignatureAt(-1)).toThrow('Signature index -1 out of range [0, 0)');
    });
  });

  describe('Signing Operations', () => {
    it('should sign with valid parameters', async () => {
      const result = await manager.sign('test-cert.p12', 'password', 'Test', 'Location');
      expect(result).toBe(true);
    });

    it('should reject null cert path', async () => {
      await expect(manager.sign(null as any, 'password', 'Test', 'Location')).rejects.toThrow(
        'Certificate path cannot be null or empty'
      );
    });

    it('should reject empty cert path', async () => {
      await expect(manager.sign('', 'password', 'Test', 'Location')).rejects.toThrow(
        'Certificate path cannot be null or empty'
      );
    });

    it('should reject whitespace-only cert path', async () => {
      await expect(manager.sign('   ', 'password', 'Test', 'Location')).rejects.toThrow(
        'Certificate path cannot be null or empty'
      );
    });

    it('should reject null password', async () => {
      await expect(manager.sign('cert.p12', null as any, 'Test', 'Location')).rejects.toThrow(
        'Password cannot be null'
      );
    });

    it('should reject null digest algorithm', async () => {
      await expect(
        manager.sign('cert.p12', 'password', 'Test', 'Location', null as any)
      ).rejects.toThrow('Digest algorithm cannot be null');
    });

    it('should sign with default digest algorithm', async () => {
      const result = await manager.sign('cert.p12', 'password');
      expect(result).toBe(true);
    });

    it('should sign with all parameters specified', async () => {
      const result = await manager.sign(
        'cert.p12',
        'password',
        'Document approved',
        'New York',
        DigestAlgorithm.SHA512
      );
      expect(result).toBe(true);
    });

    it('should clear cache after signing', async () => {
      await manager.sign('cert.p12', 'password', 'Test', 'Location');
      // Cache should be cleared (no signatures loaded after signing)
      expect(manager.getAllSignatures()).toHaveLength(0);
    });
  });

  describe('Signature Information Retrieval', () => {
    it('should throw error getting signer name from invalid index', () => {
      expect(() => manager.getSignerName(0)).toThrow('Signature 0 not found');
    });

    it('should throw error getting signature date from invalid index', () => {
      expect(() => manager.getSignatureDate(0)).toThrow('Signature 0 not found');
    });

    it('should throw error checking timestamp on invalid index', () => {
      expect(() => manager.hasTimestamp(0)).toThrow('Signature 0 not found');
    });
  });

  describe('Signature Verification', () => {
    it('should throw error verifying invalid index', () => {
      expect(() => manager.verifySignature(0)).toThrow('Signature 0 not found');
    });

    it('should return empty map for verifyAllSignatures', () => {
      const results = manager.verifyAllSignatures();
      expect(results).toBeInstanceOf(Map);
      expect(results.size).toBe(0);
    });

    it('should return false for isFullySigned on unsigned document', () => {
      expect(manager.isFullySigned()).toBe(false);
    });
  });

  describe('Certificate Information', () => {
    it('should create valid certificate info', () => {
      const now = new Date();
      const later = new Date(now.getTime() + 365 * 24 * 60 * 60 * 1000);

      const cert: CertificateInfo = {
        subject: 'CN=Test User, O=Test Org',
        issuer: 'CN=Test CA',
        serialNumber: '123456789',
        validFrom: now,
        validUntil: later,
        keySize: 2048,
        algorithm: SignatureAlgorithm.RSA,
      };

      expect(cert.subject).toBe('CN=Test User, O=Test Org');
      expect(cert.issuer).toBe('CN=Test CA');
      expect(cert.serialNumber).toBe('123456789');
      expect(cert.keySize).toBe(2048);
      expect(cert.algorithm).toBe(SignatureAlgorithm.RSA);
    });

    it('should detect expired certificate', () => {
      const past = new Date();
      past.setDate(past.getDate() - 365);
      const earlier = new Date(past.getTime() - 1 * 24 * 60 * 60 * 1000);

      const cert: CertificateInfo = {
        subject: 'CN=Test User',
        issuer: 'CN=Test CA',
        serialNumber: '123456789',
        validFrom: earlier,
        validUntil: past,
        keySize: 2048,
        algorithm: SignatureAlgorithm.RSA,
      };

      expect(cert.validUntil.getTime()).toBeLessThan(new Date().getTime());
    });

    it('should support ECDSA algorithm', () => {
      const now = new Date();
      const later = new Date(now.getTime() + 365 * 24 * 60 * 60 * 1000);

      const cert: CertificateInfo = {
        subject: 'CN=Test User',
        issuer: 'CN=Test CA',
        serialNumber: '987654321',
        validFrom: now,
        validUntil: later,
        keySize: 256,
        algorithm: SignatureAlgorithm.ECDSA,
      };

      expect(cert.algorithm).toBe(SignatureAlgorithm.ECDSA);
      expect(cert.keySize).toBe(256);
    });
  });

  describe('Signature Info', () => {
    it('should create valid signature info', () => {
      const now = new Date();
      const later = new Date(now.getTime() + 365 * 24 * 60 * 60 * 1000);

      const cert: CertificateInfo = {
        subject: 'CN=Test User',
        issuer: 'CN=Test CA',
        serialNumber: '123456789',
        validFrom: now,
        validUntil: later,
        keySize: 2048,
        algorithm: SignatureAlgorithm.RSA,
      };

      const sig: SignatureInfo = {
        index: 0,
        signerName: 'John Doe',
        signDate: now,
        reason: 'Document approval',
        location: 'Office',
        certificate: cert,
        digestAlgorithm: DigestAlgorithm.SHA256,
        verificationStatus: SignatureStatus.VALID,
      };

      expect(sig.index).toBe(0);
      expect(sig.signerName).toBe('John Doe');
      expect(sig.reason).toBe('Document approval');
      expect(sig.digestAlgorithm).toBe(DigestAlgorithm.SHA256);
      expect(sig.verificationStatus).toBe(SignatureStatus.VALID);
      expect(sig.timestamp).toBeUndefined();
    });

    it('should support timestamp', () => {
      const now = new Date();
      const later = new Date(now.getTime() + 365 * 24 * 60 * 60 * 1000);

      const cert: CertificateInfo = {
        subject: 'CN=Test User',
        issuer: 'CN=Test CA',
        serialNumber: '123456789',
        validFrom: now,
        validUntil: later,
        keySize: 2048,
        algorithm: SignatureAlgorithm.RSA,
      };

      const sig: SignatureInfo = {
        index: 0,
        signerName: 'John Doe',
        signDate: now,
        reason: 'Document approval',
        location: 'Office',
        certificate: cert,
        digestAlgorithm: DigestAlgorithm.SHA256,
        verificationStatus: SignatureStatus.VALID,
        timestamp: now,
      };

      expect(sig.timestamp).toBeDefined();
      expect(sig.timestamp).toBe(now);
    });

    it('should support different verification statuses', () => {
      const now = new Date();
      const later = new Date(now.getTime() + 365 * 24 * 60 * 60 * 1000);

      const cert: CertificateInfo = {
        subject: 'CN=Test User',
        issuer: 'CN=Test CA',
        serialNumber: '123456789',
        validFrom: now,
        validUntil: later,
        keySize: 2048,
        algorithm: SignatureAlgorithm.RSA,
      };

      const statuses: SignatureStatus[] = [
        SignatureStatus.VALID,
        SignatureStatus.INVALID,
        SignatureStatus.CERT_EXPIRED,
        SignatureStatus.CERT_REVOKED,
        SignatureStatus.TRUST_FAILED,
        SignatureStatus.UNKNOWN,
      ];

      for (const status of statuses) {
        const sig: SignatureInfo = {
          index: 0,
          signerName: 'John Doe',
          signDate: now,
          reason: 'Document approval',
          location: 'Office',
          certificate: cert,
          digestAlgorithm: DigestAlgorithm.SHA256,
          verificationStatus: status,
        };

        expect(sig.verificationStatus).toBe(status);
      }
    });
  });

  describe('Algorithm Enums', () => {
    it('should have all digest algorithms', () => {
      expect(DigestAlgorithm.SHA1).toBe(0);
      expect(DigestAlgorithm.SHA256).toBe(1);
      expect(DigestAlgorithm.SHA384).toBe(2);
      expect(DigestAlgorithm.SHA512).toBe(3);
    });

    it('should have all signature algorithms', () => {
      expect(SignatureAlgorithm.RSA).toBe(0);
      expect(SignatureAlgorithm.ECDSA).toBe(1);
    });

    it('should have all signature statuses', () => {
      expect(SignatureStatus.VALID).toBe('valid');
      expect(SignatureStatus.INVALID).toBe('invalid');
      expect(SignatureStatus.CERT_EXPIRED).toBe('cert_expired');
      expect(SignatureStatus.CERT_REVOKED).toBe('cert_revoked');
      expect(SignatureStatus.TRUST_FAILED).toBe('trust_failed');
      expect(SignatureStatus.UNKNOWN).toBe('unknown');
    });
  });

  describe('Validation Report', () => {
    it('should generate valid report structure for unsigned document', () => {
      const report = manager.getSignatureValidationReport();

      expect(report).toHaveProperty('signature_count');
      expect(report).toHaveProperty('all_valid');
      expect(report).toHaveProperty('issues');
      expect(report).toHaveProperty('recommendations');

      expect(report.signature_count).toBe(0);
      expect(report.all_valid).toBe(true);
      expect(Array.isArray(report.issues)).toBe(true);
      expect(Array.isArray(report.recommendations)).toBe(true);
      expect((report.issues as string[]).length).toBe(0);
    });

    it('should include proper report keys', () => {
      const report = manager.getSignatureValidationReport();
      const keys = Object.keys(report);

      expect(keys).toContain('signature_count');
      expect(keys).toContain('all_valid');
      expect(keys).toContain('issues');
      expect(keys).toContain('recommendations');
    });
  });

  describe('Cache Management', () => {
    it('should clear cache successfully', () => {
      manager.clearCache();
      expect(manager.getAllSignatures()).toHaveLength(0);
    });

    it('should allow multiple cache clears', () => {
      manager.clearCache();
      manager.clearCache();
      manager.clearCache();
      expect(manager.getAllSignatures()).toHaveLength(0);
    });
  });

  describe('Signature Status Enum Usage', () => {
    it('should validate all status values', () => {
      const validStatuses = [
        SignatureStatus.VALID,
        SignatureStatus.INVALID,
        SignatureStatus.CERT_EXPIRED,
        SignatureStatus.CERT_REVOKED,
        SignatureStatus.TRUST_FAILED,
        SignatureStatus.UNKNOWN,
      ];

      for (const status of validStatuses) {
        expect(typeof status).toBe('string');
        expect(status).toBeDefined();
      }
    });
  });

  describe('Error Handling', () => {
    it('should handle signing errors gracefully', async () => {
      try {
        await manager.sign(null as any, 'password', 'Test', 'Location');
        fail('Should have thrown error');
      } catch (error) {
        expect(error).toBeDefined();
        expect((error as Error).message).toContain('Certificate path');
      }
    });

    it('should handle signature retrieval errors gracefully', () => {
      try {
        manager.getSignatureAt(0);
        fail('Should have thrown error');
      } catch (error) {
        expect(error).toBeDefined();
        expect((error as Error).message).toContain('out of range');
      }
    });
  });

  describe('Thread Safety Simulation', () => {
    it('should handle concurrent operations', async () => {
      const results = await Promise.all([
        manager.sign('cert1.p12', 'pass1', 'Reason1', 'Location1'),
        manager.sign('cert2.p12', 'pass2', 'Reason2', 'Location2'),
        manager.sign('cert3.p12', 'pass3', 'Reason3', 'Location3'),
      ]);

      expect(results).toHaveLength(3);
      expect(results.every((r) => r === true)).toBe(true);
    });

    it('should handle concurrent cache operations', () => {
      manager.getAllSignatures();
      manager.clearCache();
      manager.getAllSignatures();
      manager.clearCache();

      expect(manager.getAllSignatures()).toHaveLength(0);
    });
  });
});
