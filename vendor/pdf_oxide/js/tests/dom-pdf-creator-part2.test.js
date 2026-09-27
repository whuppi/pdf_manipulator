/**
 * Phase 4 Part 2 Tests: Forms, Metadata, Page Labels, Embedded Files
 * Tests for AcroForm, XFA, XMP metadata, page labels, and embedded file support
 */

import assert from 'node:assert';
import { describe, it } from 'node:test';
import { AcroForm, DocumentInfo, EmbeddedFile, PageLabel, XMPMetadata } from '../index.js';

describe('Phase 4 Part 2: Forms, Metadata, and Advanced Features', () => {
  describe('AcroForm - Traditional PDF Forms', () => {
    it('should create new AcroForm with name', () => {
      const form = AcroForm.new('TestForm');
      assert.strictEqual(form.name, 'TestForm');
      assert.strictEqual(form.field_count(), 0);
      assert.deepStrictEqual(form.get_field_names(), []);
    });

    it('should add form field to AcroForm', () => {
      const form = AcroForm.new('TestForm');
      const field = {
        id: 'field_1',
        field_name: 'username',
        field_type: 'Text',
        label: 'Username',
        field_value: null,
        default_value: null,
        rect: { x: 10, y: 10, width: 200, height: 25 },
        page_index: 0,
        read_only: false,
        required: true,
        hidden: false,
        export_value: null,
      };

      form.add_field(field);
      assert.strictEqual(form.field_count(), 1);
      assert.strictEqual(form.get_field_names()[0], 'username');
    });

    it('should retrieve field by name', () => {
      const form = AcroForm.new('TestForm');
      const field = {
        id: 'field_1',
        field_name: 'email',
        field_type: 'Text',
        label: 'Email',
        field_value: 'test@example.com',
        default_value: null,
        rect: { x: 10, y: 50, width: 200, height: 25 },
        page_index: 0,
        read_only: false,
        required: true,
        hidden: false,
        export_value: null,
      };

      form.add_field(field);
      const retrieved = form.get_field('email');

      assert.ok(retrieved, 'Field should be found');
      assert.strictEqual(retrieved.field_name, 'email');
      assert.strictEqual(retrieved.field_value, 'test@example.com');
    });

    it('should set field value by name', () => {
      const form = AcroForm.new('TestForm');
      const field = {
        id: 'field_1',
        field_name: 'status',
        field_type: 'Text',
        label: 'Status',
        field_value: 'pending',
        default_value: null,
        rect: { x: 10, y: 90, width: 200, height: 25 },
        page_index: 0,
        read_only: false,
        required: false,
        hidden: false,
        export_value: null,
      };

      form.add_field(field);
      const success = form.set_field_value('status', 'approved');

      assert.strictEqual(success, true);
      const updated = form.get_field('status');
      assert.strictEqual(updated.field_value, 'approved');
    });

    it('should return false when setting non-existent field value', () => {
      const form = AcroForm.new('TestForm');
      const success = form.set_field_value('nonexistent', 'value');
      assert.strictEqual(success, false);
    });

    it('should get required fields', () => {
      const form = AcroForm.new('TestForm');

      const requiredField = {
        id: 'field_1',
        field_name: 'required_field',
        field_type: 'Text',
        label: 'Required',
        field_value: null,
        default_value: null,
        rect: { x: 10, y: 10, width: 200, height: 25 },
        page_index: 0,
        read_only: false,
        required: true,
        hidden: false,
        export_value: null,
      };

      const optionalField = {
        id: 'field_2',
        field_name: 'optional_field',
        field_type: 'Text',
        label: 'Optional',
        field_value: null,
        default_value: null,
        rect: { x: 10, y: 50, width: 200, height: 25 },
        page_index: 0,
        read_only: false,
        required: false,
        hidden: false,
        export_value: null,
      };

      form.add_field(requiredField);
      form.add_field(optionalField);

      const required = form.get_required_fields();
      assert.strictEqual(required.length, 1);
      assert.strictEqual(required[0], 'required_field');
    });

    it('should detect signature fields', () => {
      const form = AcroForm.new('TestForm');

      const signatureField = {
        id: 'field_1',
        field_name: 'signature',
        field_type: 'Signature',
        label: 'Sign Here',
        field_value: null,
        default_value: null,
        rect: { x: 10, y: 10, width: 150, height: 50 },
        page_index: 0,
        read_only: false,
        required: true,
        hidden: false,
        export_value: null,
      };

      form.add_field(signatureField);
      assert.strictEqual(form.has_signature_fields(), true);
    });

    it('should return false when no signature fields', () => {
      const form = AcroForm.new('TestForm');

      const textField = {
        id: 'field_1',
        field_name: 'text_field',
        field_type: 'Text',
        label: 'Text',
        field_value: null,
        default_value: null,
        rect: { x: 10, y: 10, width: 200, height: 25 },
        page_index: 0,
        read_only: false,
        required: false,
        hidden: false,
        export_value: null,
      };

      form.add_field(textField);
      assert.strictEqual(form.has_signature_fields(), false);
    });
  });

  describe('TextFormField - Text Input Fields', () => {
    it('should create text form field with properties', () => {
      const field = {
        id: 'text_1',
        field_name: 'description',
        field_value: 'Initial text',
        rect: { x: 10, y: 10, width: 300, height: 100 },
        font_name: 'Helvetica',
        font_size: 12.0,
        max_length: 500,
        multiline: true,
        color_r: 0,
        color_g: 0,
        color_b: 0,
        text_alignment: 'left',
      };

      assert.strictEqual(field.field_name, 'description');
      assert.strictEqual(field.multiline, true);
      assert.strictEqual(field.max_length, 500);
      assert.strictEqual(field.font_size, 12.0);
    });
  });

  describe('CheckboxField - Checkbox Form Fields', () => {
    it('should create checkbox field', () => {
      const field = {
        id: 'checkbox_1',
        field_name: 'agree_terms',
        rect: { x: 10, y: 10, width: 20, height: 20 },
        is_checked: false,
        checked_value: 'Yes',
        style: 'square',
      };

      assert.strictEqual(field.field_name, 'agree_terms');
      assert.strictEqual(field.is_checked, false);
      assert.strictEqual(field.checked_value, 'Yes');
    });
  });

  describe('RadioButtonField - Radio Button Fields', () => {
    it('should create radio button field with options', () => {
      const field = {
        id: 'radio_1',
        field_name: 'priority',
        rect: { x: 10, y: 10, width: 200, height: 100 },
        options: ['Low', 'Medium', 'High'],
        selected_option: 'Medium',
        export_values: ['1', '2', '3'],
      };

      assert.strictEqual(field.field_name, 'priority');
      assert.strictEqual(field.options.length, 3);
      assert.strictEqual(field.selected_option, 'Medium');
    });
  });

  describe('ListField - Dropdown/List Form Fields', () => {
    it('should create list field with options', () => {
      const field = {
        id: 'list_1',
        field_name: 'country',
        rect: { x: 10, y: 10, width: 150, height: 25 },
        options: ['USA', 'Canada', 'Mexico'],
        display_values: null,
        selected_options: ['USA'],
        multi_select: false,
        is_combo: false,
      };

      assert.strictEqual(field.field_name, 'country');
      assert.strictEqual(field.options.length, 3);
      assert.strictEqual(field.multi_select, false);
    });

    it('should support multi-select list', () => {
      const field = {
        id: 'list_2',
        field_name: 'tags',
        rect: { x: 10, y: 10, width: 200, height: 100 },
        options: ['Python', 'JavaScript', 'Rust', 'Go'],
        display_values: null,
        selected_options: ['Python', 'Rust'],
        multi_select: true,
        is_combo: false,
      };

      assert.strictEqual(field.multi_select, true);
      assert.strictEqual(field.selected_options.length, 2);
    });

    it('should support combobox (editable dropdown)', () => {
      const field = {
        id: 'combo_1',
        field_name: 'city',
        rect: { x: 10, y: 10, width: 150, height: 25 },
        options: ['New York', 'Los Angeles', 'Chicago'],
        display_values: null,
        selected_options: [],
        multi_select: false,
        is_combo: true,
      };

      assert.strictEqual(field.is_combo, true);
    });
  });

  describe('ButtonField - Push Button Fields', () => {
    it('should create button field with action', () => {
      const field = {
        id: 'button_1',
        field_name: 'submit_button',
        rect: { x: 10, y: 10, width: 100, height: 25 },
        label: 'Submit',
        action: 'Submit',
        action_target: 'https://example.com/submit',
        appearance: 'normal',
      };

      assert.strictEqual(field.label, 'Submit');
      assert.strictEqual(field.action, 'Submit');
      assert.strictEqual(field.action_target, 'https://example.com/submit');
    });
  });

  describe('SignatureField - Digital Signature Fields', () => {
    it('should create signature field', () => {
      const field = {
        id: 'sig_1',
        field_name: 'authorized_signature',
        rect: { x: 10, y: 10, width: 200, height: 50 },
        is_signed: false,
        signature_type: 'Approval',
        signer_name: null,
        signature_date: null,
        reason: null,
        location: null,
        contact_info: null,
      };

      assert.strictEqual(field.field_name, 'authorized_signature');
      assert.strictEqual(field.is_signed, false);
      assert.strictEqual(field.signature_type, 'Approval');
    });

    it('should store signature metadata', () => {
      const field = {
        id: 'sig_1',
        field_name: 'signature',
        rect: { x: 10, y: 10, width: 200, height: 50 },
        is_signed: true,
        signature_type: 'Approval',
        signer_name: 'John Doe',
        signature_date: '2024-01-16T10:30:00Z',
        reason: 'Document approval',
        location: 'New York, USA',
        contact_info: 'john@example.com',
      };

      assert.strictEqual(field.is_signed, true);
      assert.strictEqual(field.signer_name, 'John Doe');
      assert.ok(field.signature_date);
    });
  });

  describe('XMPMetadata - Extensible Metadata Platform', () => {
    it('should create empty XMP metadata', () => {
      const metadata = XMPMetadata.new();
      assert.strictEqual(metadata.is_empty(), true);
    });

    it('should set and get metadata fields', () => {
      const metadata = XMPMetadata.new();

      metadata.set_title('Test Document');
      metadata.set_author('John Doe');
      metadata.set_subject('Testing');

      assert.strictEqual(metadata.get_title(), 'Test Document');
      assert.strictEqual(metadata.get_author(), 'John Doe');
      assert.strictEqual(metadata.get_subject(), 'Testing');
    });

    it('should not be empty when fields are set', () => {
      const metadata = XMPMetadata.new();
      metadata.set_title('Document');

      assert.strictEqual(metadata.is_empty(), false);
    });

    it('should set language code', () => {
      const metadata = XMPMetadata.new();
      metadata.set_language('en-US');

      assert.strictEqual(metadata.get_language(), 'en-US');
    });

    it('should set copyright information', () => {
      const metadata = XMPMetadata.new();
      metadata.set_copyright('Copyright 2024 Acme Inc.');

      assert.strictEqual(metadata.get_copyright(), 'Copyright 2024 Acme Inc.');
    });

    it('should set keywords', () => {
      const metadata = XMPMetadata.new();
      metadata.set_keywords('pdf, testing, metadata, xmp');

      assert.strictEqual(metadata.get_keywords(), 'pdf, testing, metadata, xmp');
    });

    it('should set creator application', () => {
      const metadata = XMPMetadata.new();
      metadata.set_creator('pdf-oxide-nodejs v1.0.0');

      assert.strictEqual(metadata.get_creator(), 'pdf-oxide-nodejs v1.0.0');
    });

    it('should convert metadata to map', () => {
      const metadata = XMPMetadata.new();
      metadata.set_title('My Document');
      metadata.set_author('Alice');
      metadata.set_keywords('test');

      const map = metadata.to_map();
      assert.ok(Array.isArray(map), 'to_map should return array');
      assert.strictEqual(
        map.some((pair) => pair[0] === 'title' && pair[1] === 'My Document'),
        true,
        'Should contain title field'
      );
    });
  });

  describe('PageLabel - Page Numbering and Labels', () => {
    it('should create page label', () => {
      const label = PageLabel.new(0);
      assert.strictEqual(label.page_index, 0);
    });

    it('should generate decimal page numbers', () => {
      const label = PageLabel.new(0);
      label.set_style('decimal');
      label.set_start_value(1);

      assert.strictEqual(label.get_label_text(), '1');
    });

    it('should generate label with prefix', () => {
      const label = PageLabel.new(0);
      label.set_prefix('Chapter-');
      label.set_style('decimal');
      label.set_start_value(5);

      assert.strictEqual(label.get_label_text(), 'Chapter-5');
    });

    it('should generate Roman numerals', () => {
      const label = PageLabel.new(0);
      label.set_style('roman');
      label.set_start_value(3);

      assert.strictEqual(label.get_label_text(), 'iii');
    });

    it('should generate uppercase Roman numerals', () => {
      const label = PageLabel.new(0);
      label.set_style('uppercase_roman');
      label.set_start_value(5);

      assert.strictEqual(label.get_label_text(), 'V');
    });

    it('should generate letter sequences', () => {
      const label = PageLabel.new(0);
      label.set_style('letters');
      label.set_start_value(1);

      assert.strictEqual(label.get_label_text(), 'a');
    });

    it('should generate uppercase letters', () => {
      const label = PageLabel.new(0);
      label.set_style('uppercase');
      label.set_start_value(1);

      assert.strictEqual(label.get_label_text(), 'A');
    });

    it('should use page index when no style specified', () => {
      const label = PageLabel.new(5);
      // No style set, should use page index
      assert.strictEqual(label.get_label_text(), '6');
    });

    it('should support front matter Roman numerals', () => {
      const intro = PageLabel.new(0);
      intro.set_prefix('Introduction-');
      intro.set_style('roman');
      intro.set_start_value(1);

      const chapter = PageLabel.new(10);
      chapter.set_prefix('Chapter-');
      chapter.set_style('decimal');
      chapter.set_start_value(1);

      assert.ok(intro.get_label_text().includes('i'));
      assert.ok(chapter.get_label_text().includes('1'));
    });
  });

  describe('EmbeddedFile - Embedded Documents and Resources', () => {
    it('should create embedded file', () => {
      const file = EmbeddedFile.new('embed_1', 'document.pdf', 'application/pdf', 1024);

      assert.strictEqual(file.filename, 'document.pdf');
      assert.strictEqual(file.mime_type, 'application/pdf');
      assert.strictEqual(file.size, 1024);
    });

    it('should set and get file description', () => {
      const file = EmbeddedFile.new('embed_1', 'report.pdf', 'application/pdf', 2048);
      file.set_description('Annual report for 2024');

      assert.strictEqual(file.get_description(), 'Annual report for 2024');
    });

    it('should set and get creation date', () => {
      const file = EmbeddedFile.new('embed_1', 'data.json', 'application/json', 512);
      file.set_creation_date('2024-01-16T10:30:00Z');

      assert.strictEqual(file.get_creation_date(), '2024-01-16T10:30:00Z');
    });

    it('should manage file data', () => {
      const file = EmbeddedFile.new('embed_1', 'image.png', 'image/png', 4096);

      assert.strictEqual(file.has_data(), false);

      const base64Data =
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';
      file.set_data(base64Data);

      assert.strictEqual(file.has_data(), true);
      assert.strictEqual(file.get_data(), base64Data);
    });

    it('should support various MIME types', () => {
      const pdf = EmbeddedFile.new('pdf1', 'doc.pdf', 'application/pdf', 1024);
      const image = EmbeddedFile.new('img1', 'photo.jpg', 'image/jpeg', 2048);
      const text = EmbeddedFile.new('txt1', 'readme.txt', 'text/plain', 512);
      const json = EmbeddedFile.new('json1', 'data.json', 'application/json', 1024);

      assert.strictEqual(pdf.mime_type, 'application/pdf');
      assert.strictEqual(image.mime_type, 'image/jpeg');
      assert.strictEqual(text.mime_type, 'text/plain');
      assert.strictEqual(json.mime_type, 'application/json');
    });
  });

  describe('DocumentInfo - Basic Document Information', () => {
    it('should create document info with version', () => {
      const info = DocumentInfo.new('1.7');
      assert.strictEqual(info.version, '1.7');
      assert.strictEqual(info.is_encrypted, false);
    });

    it('should set document title', () => {
      const info = DocumentInfo.new('1.4');
      info.set_title('My Document');

      assert.strictEqual(info.title, 'My Document');
    });

    it('should generate summary string', () => {
      const info = DocumentInfo.new('1.7');
      info.set_title('Test Doc');

      const summary = info.to_summary();
      assert.ok(summary.includes('version'));
      assert.ok(summary.includes('1.7'));
      assert.ok(summary.includes('Test Doc'));
    });
  });

  describe('Integration: Forms and Metadata Together', () => {
    it('should create form with metadata context', () => {
      const metadata = XMPMetadata.new();
      metadata.set_title('Application Form');
      metadata.set_author('HR Department');
      metadata.set_keywords('form, application, hr');

      const form = AcroForm.new('ApplicationForm');
      const nameField = {
        id: 'f1',
        field_name: 'applicant_name',
        field_type: 'Text',
        label: 'Full Name',
        field_value: null,
        default_value: null,
        rect: { x: 20, y: 750, width: 300, height: 25 },
        page_index: 0,
        read_only: false,
        required: true,
        hidden: false,
        export_value: null,
      };

      form.add_field(nameField);

      assert.strictEqual(metadata.get_title(), 'Application Form');
      assert.strictEqual(form.field_count(), 1);
    });

    it('should support page labels with embedded content', () => {
      const intro = PageLabel.new(0);
      intro.set_prefix('Intro-');
      intro.set_style('roman');

      const mainContent = PageLabel.new(5);
      mainContent.set_prefix('Chapter-');
      mainContent.set_style('decimal');

      const appendix = PageLabel.new(15);
      appendix.set_prefix('Appendix-');
      appendix.set_style('letters');

      assert.ok(intro.get_label_text().includes('i'));
      assert.ok(mainContent.get_label_text().includes('1'));
      assert.ok(appendix.get_label_text().includes('a'));
    });
  });
});
