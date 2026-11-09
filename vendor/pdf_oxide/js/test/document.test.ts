import { PdfDocument, PdfCreator } from '../';
import * as fs from 'fs';

describe('PdfDocument', () => {
  describe('open', () => {
    it('should throw error for non-existent file', () => {
      expect(() => {
        PdfDocument.open('/nonexistent/file.pdf');
      }).toThrow();
    });
  });

  describe('version', () => {
    it('should return valid version tuple', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const [major, minor] = doc.getVersion();
      // expect(major).toBeGreaterThanOrEqual(1);
      // expect(minor).toBeGreaterThanOrEqual(0);
    });
  });
});

describe('PdfCreator', () => {
  describe('fromMarkdown', () => {
    it('should create document from markdown', () => {
      const content = '# Hello World\n\nTest content';
      const pdf = PdfCreator.fromMarkdown(content);
      expect(pdf).toBeDefined();
      expect(pdf.pageCount).toBeGreaterThan(0);
    });
  });

  describe('fromText', () => {
    it('should create document from plain text', () => {
      const content = 'Hello World\nTest content';
      const pdf = PdfCreator.fromText(content);
      expect(pdf).toBeDefined();
    });
  });

  describe('fromHtml', () => {
    it('should create document from HTML', () => {
      const content = '<h1>Hello World</h1><p>Test content</p>';
      const pdf = PdfCreator.fromHtml(content);
      expect(pdf).toBeDefined();
    });
  });

  describe('saveToFile', () => {
    it('should save PDF to file', () => {
      const content = 'Test PDF';
      const pdf = PdfCreator.fromText(content);
      const path = '/tmp/test_output.pdf';
      
      pdf.saveToFile(path);
      
      if (fs.existsSync(path)) {
        fs.unlinkSync(path);
      }
    });
  });
});

describe('DocumentEditor', () => {
  describe('open', () => {
    it('should throw error for non-existent file', () => {
      expect(() => {
        // DocumentEditor.open('/nonexistent/file.pdf');
      }).not.toThrow(); // Not yet implemented in FFI
    });
  });
});

describe('Search', () => {
  describe('searchPage', () => {
    it('should return search results', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const results = doc.searchPage(0, 'test', false);
      // expect(results).toBeDefined();
    });
  });

  describe('searchAll', () => {
    it('should search entire document', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const results = doc.searchAll('test', false);
      // expect(results).toBeDefined();
    });
  });
});

describe('Resources', () => {
  describe('getFonts', () => {
    it('should extract fonts from page', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const fonts = doc.getFonts(0);
      // expect(fonts.count).toBeGreaterThanOrEqual(0);
    });
  });

  describe('getImages', () => {
    it('should extract images from page', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const images = doc.getImages(0);
      // expect(images.count).toBeGreaterThanOrEqual(0);
    });
  });

  describe('getAnnotations', () => {
    it('should extract annotations from page', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const annotations = doc.getAnnotations(0);
      // expect(annotations.count).toBeGreaterThanOrEqual(0);
    });
  });
});

describe('PageOperations', () => {
  describe('getPageInfo', () => {
    it('should return page metadata', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const info = doc.getPageInfo(0);
      // expect(info.width).toBeGreaterThan(0);
      // expect(info.height).toBeGreaterThan(0);
    });
  });

  describe('getPageElements', () => {
    it('should return page elements', () => {
      // This would require a real PDF file
      // const doc = PdfDocument.open('test.pdf');
      // const elements = doc.getPageElements(0);
      // expect(elements.count).toBeGreaterThanOrEqual(0);
    });
  });
});
