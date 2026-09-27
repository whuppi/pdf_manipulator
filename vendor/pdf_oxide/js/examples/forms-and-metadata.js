/**
 * Example: Working with Forms and Metadata
 *
 * Demonstrates Phase 4 features:
 * - Creating AcroForm with various field types
 * - Setting document metadata
 * - Page labels and embedded files
 */

import {
  Pdf,
  PdfBuilder,
  AcroForm,
  XMPMetadata,
  PageLabel,
  EmbeddedFile,
} from 'pdf_oxide';

console.log('=== PDF Forms and Metadata Example ===\n');

// 1. Create document with metadata
console.log('1. Creating document with metadata...');
const doc = Pdf.from_markdown(`
# Application Form 2024

## Personal Information

Please fill out the form below.

## Employment Details

Provide your employment information.

## Signature

Sign and submit the form.
`);

// Set comprehensive metadata
const metadata = XMPMetadata.new();
metadata.set_title('Job Application Form 2024');
metadata.set_author('Human Resources');
metadata.set_subject('Employment Application');
metadata.set_keywords('application, employment, 2024, form');
metadata.set_creator('pdf-oxide-nodejs v1.0');
metadata.set_copyright('Copyright 2024 Acme Inc.');
metadata.set_language('en-US');

doc.set_metadata(metadata);
console.log('   ✓ Metadata set successfully');

// 2. Create AcroForm with various field types
console.log('\n2. Creating form with multiple field types...');
const form = AcroForm.new('JobApplicationForm');

// Text fields for personal information
const nameField = {
  id: 'field_name',
  field_name: 'applicant_name',
  field_type: 'Text',
  label: 'Full Name',
  field_value: null,
  default_value: null,
  rect: { x: 50, y: 750, width: 350, height: 25 },
  page_index: 0,
  read_only: false,
  required: true,
  hidden: false,
  export_value: null,
};

const emailField = {
  id: 'field_email',
  field_name: 'email_address',
  field_type: 'Text',
  label: 'Email Address',
  field_value: null,
  default_value: null,
  rect: { x: 50, y: 710, width: 350, height: 25 },
  page_index: 0,
  read_only: false,
  required: true,
  hidden: false,
  export_value: null,
};

// Checkbox fields
const agreeTermsField = {
  id: 'field_agree',
  field_name: 'agree_terms',
  field_type: 'Checkbox',
  label: 'I agree to the terms',
  field_value: null,
  default_value: null,
  rect: { x: 50, y: 670, width: 20, height: 20 },
  page_index: 0,
  read_only: false,
  required: true,
  hidden: false,
  export_value: 'Yes',
};

const availableField = {
  id: 'field_available',
  field_name: 'available_immediately',
  field_type: 'Checkbox',
  label: 'Available immediately',
  field_value: null,
  default_value: null,
  rect: { x: 50, y: 640, width: 20, height: 20 },
  page_index: 0,
  read_only: false,
  required: false,
  hidden: false,
  export_value: 'Yes',
};

// Radio button field for employment type
const employmentTypeField = {
  id: 'field_employment',
  field_name: 'employment_type',
  field_type: 'Radio',
  label: 'Employment Type',
  field_value: null,
  default_value: null,
  rect: { x: 50, y: 590, width: 300, height: 40 },
  page_index: 0,
  read_only: false,
  required: true,
  hidden: false,
  export_value: null,
};

// Dropdown field for position
const positionField = {
  id: 'field_position',
  field_name: 'desired_position',
  field_type: 'List',
  label: 'Desired Position',
  field_value: null,
  default_value: null,
  rect: { x: 50, y: 555, width: 300, height: 25 },
  page_index: 0,
  read_only: false,
  required: true,
  hidden: false,
  export_value: null,
};

// Signature field
const signatureField = {
  id: 'field_signature',
  field_name: 'applicant_signature',
  field_type: 'Signature',
  label: 'Your Signature',
  field_value: null,
  default_value: null,
  rect: { x: 50, y: 450, width: 300, height: 60 },
  page_index: 0,
  read_only: false,
  required: true,
  hidden: false,
  export_value: null,
};

// Add fields to form
form.add_field(nameField);
form.add_field(emailField);
form.add_field(agreeTermsField);
form.add_field(availableField);
form.add_field(employmentTypeField);
form.add_field(positionField);
form.add_field(signatureField);

console.log(`   ✓ Form created with ${form.field_count()} fields`);
console.log(`   ✓ Required fields: ${form.get_required_fields().join(', ')}`);

// 3. Apply form to document
console.log('\n3. Applying form to document...');
doc.set_forms(form);
console.log('   ✓ Form applied successfully');

// 4. Set page labels
console.log('\n4. Setting page labels...');
const pageLabel = PageLabel.new(0);
pageLabel.set_prefix('Form-');
pageLabel.set_style('decimal');
pageLabel.set_start_value(1);

doc.set_page_label(0, pageLabel);
console.log(`   ✓ Page label: ${pageLabel.get_label_text()}`);

// 5. Add embedded file (job description)
console.log('\n5. Adding embedded files...');
const jobDescription = EmbeddedFile.new(
  'job_description',
  'job_description.txt',
  'text/plain',
  512
);
jobDescription.set_description('Job description and requirements');
jobDescription.set_creation_date('2024-01-16T10:00:00Z');

const companyInfo = EmbeddedFile.new(
  'company_info',
  'company_benefits.pdf',
  'application/pdf',
  2048
);
companyInfo.set_description('Company benefits and policies');

doc.add_embedded_file(jobDescription);
doc.add_embedded_file(companyInfo);
console.log('   ✓ 2 files embedded');

// 6. Save document
console.log('\n6. Saving document...');
const outputPath = './application_form.pdf';
doc.save(outputPath);
console.log(`   ✓ Document saved to: ${outputPath}`);

// 7. Display summary
console.log('\n=== Form Summary ===');
console.log(`Name:              ${form.get_field('applicant_name').field_name}`);
console.log(`Email:             ${form.get_field('email_address').field_name}`);
console.log(`Signature Required: ${form.has_signature_fields()}`);
console.log(`Total Fields:       ${form.field_count()}`);
console.log(`Required Fields:    ${form.get_required_fields().length}`);

console.log('\n=== Metadata Summary ===');
console.log(`Title:     ${metadata.get_title()}`);
console.log(`Author:    ${metadata.get_author()}`);
console.log(`Subject:   ${metadata.get_subject()}`);
console.log(`Keywords:  ${metadata.get_keywords()}`);
console.log(`Language:  ${metadata.get_language()}`);

console.log('\n✨ Example completed successfully!\n');
