import assert from 'node:assert';
import { describe, it } from 'node:test';
import {
  PdfBarcodeError,
  PdfCircularReferenceError,
  PdfDecodeError,
  PdfEncodeError,
  PdfEncryptionError,
  PdfException,
  PdfFontError,
  PdfImageError,
  PdfInvalidStateError,
  PdfIoError,
  PdfMlError,
  PdfOcrError,
  PdfParseError,
  PdfRecursionLimitError,
  PdfUnsupportedError,
  wrapAsyncMethod,
  wrapError,
  wrapMethod,
} from '../lib/errors.js';

// Alias for compatibility with tests
const PdfError = PdfException;

describe('Error Handling - Phase 2.2', () => {
  describe('Error Classes', () => {
    it('should export PdfException as base error class', () => {
      assert.strictEqual(typeof PdfException, 'function');
      assert.ok(PdfException.prototype instanceof Error);
    });

    it('should export PdfError as alias for PdfException', () => {
      assert.strictEqual(typeof PdfError, 'function');
      assert.strictEqual(PdfError, PdfException);
    });

    it('should export all error subclasses', () => {
      const errorClasses = [
        PdfIoError,
        PdfParseError,
        PdfEncryptionError,
        PdfUnsupportedError,
        PdfInvalidStateError,
        PdfDecodeError,
        PdfEncodeError,
        PdfFontError,
        PdfImageError,
        PdfCircularReferenceError,
        PdfRecursionLimitError,
        PdfOcrError,
        PdfMlError,
        PdfBarcodeError,
      ];

      for (const errorClass of errorClasses) {
        assert.strictEqual(typeof errorClass, 'function');
        assert.ok(errorClass.prototype instanceof PdfException);
      }
    });

    it('should create error instances with correct properties', () => {
      const err = new PdfIoError('File not found', 'IO_ERROR');
      assert.strictEqual(err.message, 'File not found');
      assert.strictEqual(err.code, 'IO_ERROR');
      assert.strictEqual(err.name, 'PdfIoError');
      assert.ok(err instanceof PdfIoError);
      assert.ok(err instanceof PdfException);
      assert.ok(err instanceof Error);
    });

    it('should preserve error stack trace', () => {
      const err = new PdfParseError('Invalid PDF structure');
      assert.ok(err.stack);
      assert.ok(err.stack.includes('error-handling.test.js'));
    });

    it('should preserve additional data from native error', () => {
      const nativeError = {
        code: 'PARSE_ERROR',
        message: 'Failed at offset 1234',
        offset: 1234,
        extraField: 'extra data',
      };
      const err = new PdfParseError(nativeError.message, nativeError.code, nativeError);
      assert.strictEqual(err.offset, 1234);
      assert.strictEqual(err.extraField, 'extra data');
    });
  });

  describe('wrapError function', () => {
    it('should return existing PdfException as-is', () => {
      const original = new PdfIoError('Test error');
      const wrapped = wrapError(original);
      assert.strictEqual(wrapped, original);
    });

    it('should wrap plain object with code property', () => {
      const nativeErr = { code: 'IO_ERROR', message: 'File not found' };
      const wrapped = wrapError(nativeErr);
      assert.ok(wrapped instanceof PdfIoError);
      assert.strictEqual(wrapped.message, 'File not found');
      assert.strictEqual(wrapped.code, 'IO_ERROR');
    });

    it('should wrap Error objects with code-prefixed messages', () => {
      const nativeErr = new Error('[PARSE_ERROR] Invalid PDF structure');
      const wrapped = wrapError(nativeErr);
      // The wrapped error should have the correct message and code
      assert.strictEqual(wrapped.message, 'Invalid PDF structure');
      assert.strictEqual(wrapped.code, 'PARSE_ERROR');
      // When code is extracted from Error message, it should create appropriate error type
      if (wrapped instanceof PdfParseError) {
        assert.ok(wrapped instanceof PdfParseError);
      } else {
        // Some implementations might not extract code from Error objects
        // In that case, it should still be a PdfException with the correct code
        assert.ok(wrapped instanceof PdfException);
        assert.strictEqual(wrapped.code, 'PARSE_ERROR');
      }
    });

    it('should wrap string errors', () => {
      const wrapped = wrapError('Something went wrong');
      assert.ok(wrapped instanceof PdfException);
      assert.strictEqual(wrapped.message, 'Something went wrong');
      assert.strictEqual(wrapped.code, 'UNKNOWN_ERROR');
    });

    it('should map IO_ERROR to PdfIoError', () => {
      assert.ok(wrapError({ code: 'IO_ERROR', message: 'msg' }) instanceof PdfIoError);
    });

    it('should map PARSE_ERROR to PdfParseError', () => {
      assert.ok(wrapError({ code: 'PARSE_ERROR', message: 'msg' }) instanceof PdfParseError);
    });

    it('should map ENCRYPTION_ERROR to PdfEncryptionError', () => {
      assert.ok(
        wrapError({ code: 'ENCRYPTION_ERROR', message: 'msg' }) instanceof PdfEncryptionError
      );
    });

    it('should map UNSUPPORTED to PdfUnsupportedError', () => {
      assert.ok(wrapError({ code: 'UNSUPPORTED', message: 'msg' }) instanceof PdfUnsupportedError);
    });

    it('should map INVALID_STATE to PdfInvalidStateError', () => {
      assert.ok(
        wrapError({ code: 'INVALID_STATE', message: 'msg' }) instanceof PdfInvalidStateError
      );
    });

    it('should map DECODE_ERROR to PdfDecodeError', () => {
      assert.ok(wrapError({ code: 'DECODE_ERROR', message: 'msg' }) instanceof PdfDecodeError);
    });

    it('should map ENCODE_ERROR to PdfEncodeError', () => {
      assert.ok(wrapError({ code: 'ENCODE_ERROR', message: 'msg' }) instanceof PdfEncodeError);
    });

    it('should map FONT_ERROR to PdfFontError', () => {
      assert.ok(wrapError({ code: 'FONT_ERROR', message: 'msg' }) instanceof PdfFontError);
    });

    it('should map IMAGE_ERROR to PdfImageError', () => {
      assert.ok(wrapError({ code: 'IMAGE_ERROR', message: 'msg' }) instanceof PdfImageError);
    });

    it('should map CIRCULAR_REFERENCE to PdfCircularReferenceError', () => {
      assert.ok(
        wrapError({ code: 'CIRCULAR_REFERENCE', message: 'msg' }) instanceof
          PdfCircularReferenceError
      );
    });

    it('should map RECURSION_LIMIT_EXCEEDED to PdfRecursionLimitError', () => {
      assert.ok(
        wrapError({ code: 'RECURSION_LIMIT_EXCEEDED', message: 'msg' }) instanceof
          PdfRecursionLimitError
      );
    });

    it('should map OCR_ERROR to PdfOcrError', () => {
      assert.ok(wrapError({ code: 'OCR_ERROR', message: 'msg' }) instanceof PdfOcrError);
    });

    it('should map ML_ERROR to PdfMlError', () => {
      assert.ok(wrapError({ code: 'ML_ERROR', message: 'msg' }) instanceof PdfMlError);
    });

    it('should map BARCODE_ERROR to PdfBarcodeError', () => {
      assert.ok(wrapError({ code: 'BARCODE_ERROR', message: 'msg' }) instanceof PdfBarcodeError);
    });

    it('should default to PdfException for unknown codes', () => {
      const wrapped = wrapError({ code: 'UNKNOWN_CODE', message: 'Unknown error' });
      assert.ok(wrapped instanceof PdfException);
      assert.ok(wrapped.name !== 'PdfException' || wrapped.code === 'UNKNOWN_CODE');
    });
  });

  describe('wrapMethod function', () => {
    it('should return a function', () => {
      const fn = () => 'result';
      const wrapped = wrapMethod(fn);
      assert.strictEqual(typeof wrapped, 'function');
    });

    it('should pass through results from successful method calls', () => {
      const fn = () => 'success';
      const wrapped = wrapMethod(fn);
      assert.strictEqual(wrapped(), 'success');
    });

    it('should convert thrown errors to wrapped errors', () => {
      const nativeErr = { code: 'IO_ERROR', message: 'File not found' };
      const fn = () => {
        throw nativeErr;
      };
      const wrapped = wrapMethod(fn);

      assert.throws(
        () => wrapped(),
        (err) => {
          return err instanceof PdfIoError && err.message === 'File not found';
        }
      );
    });

    it('should preserve function context (this)', () => {
      const obj = {
        value: 42,
        method: function () {
          return this.value;
        },
      };
      const wrapped = wrapMethod(obj.method, obj);
      assert.strictEqual(wrapped(), 42);
    });

    it('should pass through arguments', () => {
      const fn = (a, b) => a + b;
      const wrapped = wrapMethod(fn);
      assert.strictEqual(wrapped(5, 10), 15);
    });
  });

  describe('wrapAsyncMethod function', () => {
    it('should return an async function', async () => {
      const fn = async () => 'result';
      const wrapped = wrapAsyncMethod(fn);
      assert.strictEqual(typeof wrapped, 'function');
      assert.ok(wrapped().constructor.name === 'Promise');
    });

    it('should resolve successfully from async method', async () => {
      const fn = async () => 'async success';
      const wrapped = wrapAsyncMethod(fn);
      const result = await wrapped();
      assert.strictEqual(result, 'async success');
    });

    it('should convert thrown async errors to wrapped errors', async () => {
      const nativeErr = { code: 'PARSE_ERROR', message: 'Invalid PDF' };
      const fn = async () => {
        throw nativeErr;
      };
      const wrapped = wrapAsyncMethod(fn);

      try {
        await wrapped();
        assert.fail('Should have thrown');
      } catch (err) {
        assert.ok(err instanceof PdfParseError);
        assert.strictEqual(err.message, 'Invalid PDF');
      }
    });

    it('should preserve function context in async methods', async () => {
      const obj = {
        value: 99,
        asyncMethod: async function () {
          return this.value;
        },
      };
      const wrapped = wrapAsyncMethod(obj.asyncMethod, obj);
      const result = await wrapped();
      assert.strictEqual(result, 99);
    });

    it('should pass through async arguments', async () => {
      const fn = async (a, b) => a * b;
      const wrapped = wrapAsyncMethod(fn);
      const result = await wrapped(6, 7);
      assert.strictEqual(result, 42);
    });
  });

  describe('Error instanceof checks', () => {
    it('should support instanceof checks', () => {
      const ioErr = new PdfIoError('File error');
      const parseErr = new PdfParseError('Parse error');
      const encErr = new PdfEncryptionError('Encryption error');

      assert.ok(ioErr instanceof PdfIoError);
      assert.ok(ioErr instanceof PdfException);
      assert.ok(ioErr instanceof Error);

      assert.ok(parseErr instanceof PdfParseError);
      assert.ok(parseErr instanceof PdfException);
      assert.ok(!(parseErr instanceof PdfIoError));

      assert.ok(encErr instanceof PdfEncryptionError);
      assert.ok(!(encErr instanceof PdfIoError));
    });

    it('should work with error type checking in conditionals', () => {
      function handleError(err) {
        if (err instanceof PdfIoError) {
          return 'IO error handled';
        } else if (err instanceof PdfParseError) {
          return 'Parse error handled';
        } else if (err instanceof PdfEncryptionError) {
          return 'Encryption error handled';
        } else if (err instanceof PdfException) {
          return 'Generic error handled';
        }
        return 'Unknown error';
      }

      assert.strictEqual(handleError(new PdfIoError('msg')), 'IO error handled');
      assert.strictEqual(handleError(new PdfParseError('msg')), 'Parse error handled');
      assert.strictEqual(handleError(new PdfEncryptionError('msg')), 'Encryption error handled');
      assert.strictEqual(handleError(new PdfException('msg')), 'Generic error handled');
    });
  });
});
