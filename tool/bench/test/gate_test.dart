import 'package:pdf_manipulator_bench/gate.dart';
import 'package:test/test.dart';

const _header =
    'case,kind,engine,preset,in_bytes,out_bytes,ratio,ms,ms_median,'
    'deterministic,pages_in,pages_out,worst_rmse,worst_rmse_page,worst_ssim,'
    'worst_ssim_page,text_equal,error';

/// One CSV row with the columns the gate reads; the rest are filler.
String _row(
  String engine,
  String preset,
  int? out, {
  String id = 'doc',
  String pagesOut = '3',
  String rmse = '0.0000',
  String ssim = '1.00000',
  String text = 'true',
  String error = '',
}) =>
    '$id,real,$engine,$preset,1000,${out ?? ''},0.5000,10,10,yes,3,'
    '${out == null ? '' : pagesOut},${out == null ? '' : rmse},0,'
    '${out == null ? '' : ssim},0,'
    '${out == null ? '' : text},"$error"';

GateReport _gate(List<String> rows) =>
    evaluateGate(parseBenchCsv([_header, ...rows].join('\n')));

void main() {
  test('all rows pass', () {
    final report = _gate([
      _row('pdf_manipulator', 'lossless', 800),
      _row('pdf_document', 'lossless', 900),
      _row('pdf_manipulator', 'screen', 300, rmse: '1.2000'),
      _row('pdf_document', 'screen', 300),
    ]);
    expect(report.failures, isEmpty);
    expect(report.table, contains('| doc | screen | 300 | 300 | 1.000 |'));
  });

  test('pdf_document smaller on one preset fails that preset only', () {
    final report = _gate([
      _row('pdf_manipulator', 'lossless', 800),
      _row('pdf_document', 'lossless', 900),
      _row('pdf_manipulator', 'ebook', 500),
      _row('pdf_document', 'ebook', 450),
    ]);
    expect(report.failures, hasLength(1));
    expect(report.failures.single, contains('doc ebook'));
    expect(
      report.failures.single,
      contains('ours 500 bytes, theirs 450 bytes'),
    );
  });

  test('pdf_document smaller but worse is a note, not a failure', () {
    final report = _gate([
      _row('pdf_manipulator', 'ebook', 2589871),
      _row('pdf_document', 'ebook', 2439955, rmse: '8.0849', ssim: '0.91957'),
    ]);
    expect(report.failures, isEmpty);
    expect(report.notes.single, contains('smaller but worse'));
  });

  test('pdf_document smaller at equal quality on a lossy preset fails', () {
    final report = _gate([
      _row('pdf_manipulator', 'screen', 500, rmse: '4.0000', ssim: '0.95000'),
      _row('pdf_document', 'screen', 450, rmse: '3.0000', ssim: '0.96000'),
    ]);
    expect(report.failures, hasLength(1));
    expect(report.failures.single, contains('same or better quality'));
  });

  test('pdf_document smaller with lost text is a note', () {
    final report = _gate([
      _row('pdf_manipulator', 'lossless', 800),
      _row('pdf_document', 'lossless', 700, text: 'false'),
    ]);
    expect(report.failures, isEmpty);
    expect(report.notes, hasLength(1));
  });

  test('a lossless rmse above zero fails', () {
    final report = _gate([
      _row('pdf_manipulator', 'lossless', 800, rmse: '0.0100'),
      _row('pdf_document', 'lossless', 900),
    ]);
    expect(report.failures, hasLength(1));
    expect(report.failures.single, contains('doc lossless'));
    expect(report.failures.single, contains('rmse 0.0100'));
  });

  test('a lossy rmse above zero is allowed', () {
    final report = _gate([
      _row('pdf_manipulator', 'printer', 400, rmse: '2.5000'),
      _row('pdf_document', 'printer', 410),
    ]);
    expect(report.failures, isEmpty);
  });

  test('lost pages and unequal text fail', () {
    final report = _gate([
      _row('pdf_manipulator', 'screen', 300, pagesOut: '2', text: 'false'),
      _row('pdf_document', 'screen', 350),
    ]);
    expect(report.failures, hasLength(2));
  });

  test('an error in our row fails', () {
    final report = _gate([
      _row('pdf_manipulator', 'screen', null, error: 'boom'),
      _row('pdf_document', 'screen', 350),
    ]);
    expect(report.failures, hasLength(1));
    expect(report.failures.single, contains('doc screen'));
    expect(report.failures.single, contains('boom'));
    expect(report.failures.single, contains('ours - bytes, theirs 350 bytes'));
  });

  test('an error in their row only passes with a note', () {
    final report = _gate([
      _row('pdf_manipulator', 'screen', 300),
      _row('pdf_document', 'screen', null, error: 'cannot parse'),
    ]);
    expect(report.failures, isEmpty);
    expect(report.notes, hasLength(1));
    expect(report.notes.single, contains('cannot parse'));
  });

  test('an error string with commas is one field', () {
    final report = _gate([
      _row('pdf_manipulator', 'screen', null, error: 'bad xref, offset 12, x'),
      _row('pdf_document', 'screen', 350),
    ]);
    expect(report.failures, hasLength(1));
    expect(report.failures.single, contains('bad xref, offset 12, x'));
  });
}
