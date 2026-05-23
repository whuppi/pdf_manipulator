import 'dart:developer';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'package:file_picker/file_picker.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

// ─── In-memory PdfSource / PdfSink for the example app ────────────

class _MemorySource implements PdfSource {
  _MemorySource(this._data);
  final Uint8List _data;
  @override
  int get length => _data.length;
  @override
  Uint8List readAt(int offset, int count) {
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.sublistView(_data, offset, end);
  }
}

class _MemorySink implements PdfSink {
  final _builder = BytesBuilder(copy: false);
  @override
  void write(Uint8List chunk) => _builder.add(chunk);
  Uint8List takeBytes() => _builder.takeBytes();
}

void main() => runApp(const MyApp());

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'PDF Manipulator',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorSchemeSeed: Colors.deepPurple,
      ),
      darkTheme: ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorSchemeSeed: Colors.deepPurple,
      ),
      themeMode: ThemeMode.system,
      home: const HomePage(),
    );
  }
}

// ─── Home — tabbed navigation ──────────────────────────────────────

class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  int _tabIndex = 0;

  final _pages = const <Widget>[
    _SinglePdfTab(),
    _MergeTab(),
    _ImagesToPdfTab(),
    _EditorTab(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: _pages[_tabIndex],
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tabIndex,
        onDestinationSelected: (i) => setState(() => _tabIndex = i),
        destinations: const [
          NavigationDestination(icon: Icon(Icons.picture_as_pdf), label: 'Operations'),
          NavigationDestination(icon: Icon(Icons.merge), label: 'Merge'),
          NavigationDestination(icon: Icon(Icons.image), label: 'Images→PDF'),
          NavigationDestination(icon: Icon(Icons.edit_document), label: 'Editor'),
        ],
      ),
    );
  }
}

// ─── Shared helpers ────────────────────────────────────────────────

String fmtSize(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / 1024 / 1024).toStringAsFixed(2)} MB';
}

/// Pick one or more PDFs. Returns bytes directly — no dart:io, works on web.
Future<List<Uint8List>?> pickPdfBytes({bool multiple = false}) async {
  final result = await FilePicker.platform.pickFiles(
    allowMultiple: multiple,
    type: FileType.custom,
    allowedExtensions: ['pdf'],
    withData: true,
  );
  if (result == null) return null;
  final out = <Uint8List>[];
  for (final f in result.files) {
    if (f.bytes != null) out.add(f.bytes!);
  }
  return out.isEmpty ? null : out;
}

/// Pick one or more images. Returns bytes directly.
Future<List<PickedFile>?> pickImageBytes() async {
  final result = await FilePicker.platform.pickFiles(
    allowMultiple: true,
    type: FileType.image,
    withData: true,
  );
  if (result == null) return null;
  final out = <PickedFile>[];
  for (final f in result.files) {
    if (f.bytes != null) out.add(PickedFile(name: f.name, bytes: f.bytes!, size: f.size));
  }
  return out.isEmpty ? null : out;
}

/// Save bytes — uses FilePicker.platform.saveFile which works on all platforms.
Future<String?> saveBytes(Uint8List bytes, String name) async {
  final result = await FilePicker.platform.saveFile(
    dialogTitle: 'Save $name',
    fileName: name,
    bytes: bytes,
  );
  return result;
}

class _StatusBar extends StatelessWidget {
  final bool loading;
  final String? message;
  final VoidCallback? onDismiss;
  const _StatusBar({this.loading = false, this.message, this.onDismiss});

  @override
  Widget build(BuildContext context) {
    if (!loading && message == null) return const SizedBox.shrink();
    final cs = Theme.of(context).colorScheme;
    final isError = message?.startsWith('Error') == true;
    final bg = loading ? cs.primaryContainer : (isError ? cs.errorContainer : cs.tertiaryContainer);
    final fg = loading ? cs.onPrimaryContainer : (isError ? cs.onErrorContainer : cs.onTertiaryContainer);

    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      color: bg,
      child: Row(
        children: [
          if (loading) ...[
            SizedBox(width: 14, height: 14, child: CircularProgressIndicator(strokeWidth: 2, color: fg)),
            const SizedBox(width: 12),
          ],
          Expanded(child: Text(message ?? '', style: TextStyle(color: fg, fontSize: 13), maxLines: 4)),
          if (!loading && onDismiss != null)
            GestureDetector(onTap: onDismiss, child: Icon(Icons.close, size: 16, color: fg)),
        ],
      ),
    );
  }
}

class _FileInfoCard extends StatelessWidget {
  final String fileName;
  final int fileSize;
  final PdfDoc doc;

  final VoidCallback onChangePdf;

  const _FileInfoCard({
    required this.fileName,
    required this.fileSize,
    required this.doc,

    required this.onChangePdf,
  });

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(Icons.picture_as_pdf, color: cs.primary, size: 28),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(fileName, style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 15),
                          overflow: TextOverflow.ellipsis),
                      const SizedBox(height: 2),
                      Text('${doc.pageCount} pages  •  ${fmtSize(fileSize)}  •  v${doc.version}',
                          style: TextStyle(fontSize: 12, color: cs.onSurfaceVariant)),
                    ],
                  ),
                ),
                IconButton.filledTonal(
                  onPressed: onChangePdf,
                  icon: const Icon(Icons.swap_horiz, size: 20),
                  tooltip: 'Change PDF',
                ),
              ],
            ),
            if (doc.title != null || doc.author != null || doc.isEncrypted) ...[
              const SizedBox(height: 10),
              Wrap(
                spacing: 6,
                runSpacing: 4,
                children: [
                  if (doc.title != null)
                    _tag(context, doc.title!),
                  if (doc.author != null)
                    _tag(context, 'by ${doc.author}'),
                  if (doc.isEncrypted)
                    _tag(context, 'Encrypted', isWarning: true),
                  if (doc.isTagged)
                    _tag(context, 'Tagged'),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _tag(BuildContext context, String text, {bool isWarning = false}) {
    final cs = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: isWarning ? cs.errorContainer : cs.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Text(text, style: TextStyle(fontSize: 11, color: isWarning ? cs.onErrorContainer : cs.onSurfaceVariant)),
    );
  }
}

// ─── Tab 1: Single-PDF operations ──────────────────────────────────

class _SinglePdfTab extends StatefulWidget {
  const _SinglePdfTab();
  @override
  State<_SinglePdfTab> createState() => _SinglePdfTabState();
}

class _SinglePdfTabState extends State<_SinglePdfTab> {
  final _pdf = Pdf();

  String? _fileName;
  int? _fileSize;
  Uint8List? _fileBytes;
  PdfDoc? _pdfDoc;
  bool _loading = false;
  String? _status;

  @override
  void dispose() {
    _pdf.dispose();
    super.dispose();
  }

  Future<void> _pickPdf() async {
    final result = await pickPdfBytes();
    if (result == null) return;
    final bytes = result.first;

    setState(() { _loading = true; _status = 'Opening...'; });
    try {
      final source = _MemorySource(bytes);
      final doc = await _pdf.open(source);
      setState(() {
        _fileName = 'Selected PDF';
        _fileSize = bytes.length;
        _fileBytes = bytes;
        _pdfDoc = doc;
        _status = null;
      });
    } on PdfError catch (e) {
      setState(() => _status = 'Error: $e');
    } finally {
      setState(() => _loading = false);
    }
  }

  Future<void> _run(String label, Future<void> Function() op) async {
    setState(() { _loading = true; _status = '$label...'; });
    try {
      await op();
    } on PdfError catch (e) {
      setState(() => _status = 'Error: $e');
      log('$label: $e');
    } catch (e) {
      setState(() => _status = 'Error: $e');
      log('$label: $e');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _saveAndReport(Uint8List result, String name) async {
    final path = await saveBytes(result, name);
    if (path != null) {
      setState(() => _status = 'Saved $name (${fmtSize(result.length)})');
    } else {
      setState(() => _status = 'Cancelled');
    }
  }

  @override
  Widget build(BuildContext context) {
    final hasPdf = _fileBytes != null;

    return Scaffold(
      appBar: AppBar(title: const Text('PDF Operations')),
      body: Column(
        children: [
          _StatusBar(loading: _loading, message: _status,
              onDismiss: () => setState(() => _status = null)),
          Expanded(child: hasPdf ? _buildOps() : _buildLanding()),
        ],
      ),
    );
  }

  Widget _buildLanding() {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.picture_as_pdf_outlined, size: 72,
                color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.6)),
            const SizedBox(height: 20),
            const Text('Pick a PDF to get started',
                style: TextStyle(fontSize: 18, fontWeight: FontWeight.w500)),
            const SizedBox(height: 6),
            Text('All operations run off the main thread',
                style: TextStyle(fontSize: 13, color: Theme.of(context).colorScheme.onSurfaceVariant)),
            const SizedBox(height: 28),
            FilledButton.icon(
              onPressed: _loading ? null : _pickPdf,
              icon: const Icon(Icons.file_open),
              label: const Text('Open PDF'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildOps() {
    final doc = _pdfDoc!;
    final bytes = _fileBytes!;

    return ListView(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      children: [
        _FileInfoCard(
          fileName: _fileName!,
          fileSize: _fileSize!,
          doc: doc,
          onChangePdf: _pickPdf,
        ),
        const SizedBox(height: 4),

        // Page info expandable
        _ExpandableSection(
          icon: Icons.info_outline,
          title: 'Page Details',
          child: Column(
            children: [
              for (var i = 0; i < doc.pageCount && i < 30; i++)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 3, horizontal: 12),
                  child: Row(
                    children: [
                      SizedBox(width: 32, child: Text('${i + 1}',
                          style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 13))),
                      Expanded(
                        child: Text(
                          '${doc.pages[i].effectiveWidth.toStringAsFixed(0)} × '
                          '${doc.pages[i].effectiveHeight.toStringAsFixed(0)} pt'
                          '${doc.pages[i].rotation != 0 ? '  •  ${doc.pages[i].rotation}°' : ''}',
                          style: const TextStyle(fontSize: 13),
                        ),
                      ),
                    ],
                  ),
                ),
              if (doc.pageCount > 30)
                Padding(
                  padding: const EdgeInsets.all(8),
                  child: Text('+ ${doc.pageCount - 30} more',
                      style: TextStyle(fontSize: 12, color: Theme.of(context).colorScheme.onSurfaceVariant)),
                ),
            ],
          ),
        ),
        const SizedBox(height: 12),

        // ── STRUCTURAL ──
        _SectionLabel('Structural'),
        _OpTile(icon: Icons.content_cut, title: 'Split by page count',
          subtitle: 'Every 2 pages', loading: _loading,
          onRun: () => _run('Split', () async {
            final sinks = <_MemorySink>[];
            await _pdf.split(_MemorySource(bytes), (index) {
              final s = _MemorySink();
              sinks.add(s);
              return s;
            }, every: 2);
            final chunks = sinks.map((s) => s.takeBytes()).toList();
            setState(() => _status = '${chunks.length} chunks: ${chunks.map((c) => fmtSize(c.length)).join(', ')}');
          })),
        _OpTile(icon: Icons.straighten, title: 'Split by size',
          subtitle: 'Max 500 KB per chunk', loading: _loading,
          onRun: () => _run('Split by size', () async {
            final sinks = <_MemorySink>[];
            await _pdf.splitBySize(_MemorySource(bytes), (index) {
              final s = _MemorySink();
              sinks.add(s);
              return s;
            }, maxBytes: 500000);
            final chunks = sinks.map((s) => s.takeBytes()).toList();
            setState(() => _status = '${chunks.length} chunks: ${chunks.map((c) => fmtSize(c.length)).join(', ')}');
          })),
        if (doc.pageCount >= 3)
          _OpTile(icon: Icons.file_copy_outlined, title: 'Extract pages 1–2',
            subtitle: 'First two pages as new PDF', loading: _loading,
            onRun: () => _run('Extract', () async {
              final sink = _MemorySink();
              await _pdf.extractPages(_MemorySource(bytes), sink, pages: [0, 1]);
              await _saveAndReport(sink.takeBytes(), 'extracted.pdf');
            })),
        if (doc.pageCount >= 2)
          _OpTile(icon: Icons.delete_outline, title: 'Delete first page',
            subtitle: 'Remove page 1', loading: _loading,
            onRun: () => _run('Delete', () async {
              final sink = _MemorySink();
              await _pdf.deletePages(_MemorySource(bytes), sink, pages: [0]);
              await _saveAndReport(sink.takeBytes(), 'deleted.pdf');
            })),
        if (doc.pageCount >= 3)
          _OpTile(icon: Icons.swap_vert, title: 'Reverse page order',
            subtitle: 'All pages in reverse', loading: _loading,
            onRun: () => _run('Reverse', () async {
              final order = List.generate(doc.pageCount, (i) => doc.pageCount - 1 - i);
              final sink = _MemorySink();
              await _pdf.reorderPages(_MemorySource(bytes), sink, order: order);
              await _saveAndReport(sink.takeBytes(), 'reversed.pdf');
            })),
        if (doc.pageCount >= 2)
          _OpTile(icon: Icons.move_down, title: 'Move page 1 → last',
            subtitle: 'Move first page to end', loading: _loading,
            onRun: () => _run('Move page', () async {
              final sink = _MemorySink();
              await _pdf.movePage(_MemorySource(bytes), sink, from: 0, to: doc.pageCount - 1);
              await _saveAndReport(sink.takeBytes(), 'moved.pdf');
            })),
        const SizedBox(height: 12),

        // ── ROTATION ──
        _SectionLabel('Rotation'),
        _OpTile(icon: Icons.rotate_right, title: 'Rotate all 90°',
          subtitle: 'Clockwise', loading: _loading,
          onRun: () => _run('Rotate all', () async {
            final sink = _MemorySink();
            await _pdf.rotateAllPages(_MemorySource(bytes), sink, degrees: 90);
            await _saveAndReport(sink.takeBytes(), 'rotated_all_90.pdf');
          })),
        _OpTile(icon: Icons.rotate_left, title: 'Rotate page 1 → 180°',
          subtitle: 'Flip first page upside down', loading: _loading,
          onRun: () => _run('Rotate page 1', () async {
            final sink = _MemorySink();
            await _pdf.rotatePages(_MemorySource(bytes), sink, pages: {0: 180});
            await _saveAndReport(sink.takeBytes(), 'rotated_p1_180.pdf');
          })),
        if (doc.pageCount >= 2)
          _OpTile(icon: Icons.rotate_90_degrees_ccw, title: 'Rotate page 2 → 270°',
            subtitle: 'Counter-clockwise', loading: _loading,
            onRun: () => _run('Rotate page 2', () async {
              final sink = _MemorySink();
              await _pdf.rotatePages(_MemorySource(bytes), sink, pages: {1: 270});
              await _saveAndReport(sink.takeBytes(), 'rotated_p2_270.pdf');
            })),
        const SizedBox(height: 12),

        // ── COMPRESSION ──
        _SectionLabel('Compression'),
        _OpTile(icon: Icons.compress, title: 'Compress (quality 75)',
          subtitle: 'Stream recompression + GC + image optimization', loading: _loading,
          onRun: () => _run('Compress', () async {
            final sink = _MemorySink();
            await _pdf.compress(_MemorySource(bytes), sink, imageQuality: 75);
            final r = sink.takeBytes();
            final pct = ((1 - r.length / bytes.length) * 100).toStringAsFixed(1);
            setState(() => _status = '${fmtSize(bytes.length)} → ${fmtSize(r.length)} ($pct% ${r.length < bytes.length ? "smaller" : "larger"})');
            if (r.length < bytes.length) await _saveAndReport(r, 'compressed.pdf');
          })),
        _OpTile(icon: Icons.compress, title: 'Compress (quality 30)',
          subtitle: 'Aggressive — lower quality, smaller size', loading: _loading,
          onRun: () => _run('Compress aggressively', () async {
            final sink = _MemorySink();
            await _pdf.compress(_MemorySource(bytes), sink, imageQuality: 30);
            final r = sink.takeBytes();
            final pct = ((1 - r.length / bytes.length) * 100).toStringAsFixed(1);
            setState(() => _status = '${fmtSize(bytes.length)} → ${fmtSize(r.length)} ($pct%)');
          })),
        const SizedBox(height: 12),

        // ── WATERMARK ──
        _SectionLabel('Watermark'),
        _OpTile(icon: Icons.water_drop, title: 'Watermark "DRAFT"',
          subtitle: 'Semi-transparent on all pages', loading: _loading,
          onRun: () => _run('Watermark', () async {
            final sink = _MemorySink();
            await _pdf.watermark(_MemorySource(bytes), sink, text: 'DRAFT',
                style: const PdfWatermarkStyle(opacity: 0.3));
            await _saveAndReport(sink.takeBytes(), 'watermarked_draft.pdf');
          })),
        _OpTile(icon: Icons.water_drop_outlined, title: 'Watermark "CONFIDENTIAL"',
          subtitle: 'Large, rotated, red-ish', loading: _loading,
          onRun: () => _run('Watermark', () async {
            final sink = _MemorySink();
            await _pdf.watermark(_MemorySource(bytes), sink, text: 'CONFIDENTIAL',
                style: const PdfWatermarkStyle(opacity: 0.2, fontSize: 60, rotation: 45));
            await _saveAndReport(sink.takeBytes(), 'watermarked_confidential.pdf');
          })),
        const SizedBox(height: 12),

        // ── SECURITY ──
        _SectionLabel('Security'),
        _OpTile(icon: Icons.lock, title: 'Encrypt',
          subtitle: 'Owner password: secret123', loading: _loading,
          onRun: () => _run('Encrypt', () async {
            final sink = _MemorySink();
            await _pdf.encrypt(_MemorySource(bytes), sink,
                encryption: const PdfEncryptionConfig(ownerPassword: 'secret123'));
            await _saveAndReport(sink.takeBytes(), 'encrypted.pdf');
          })),
        _OpTile(icon: Icons.lock_open, title: 'Decrypt',
          subtitle: 'Try password: secret123', loading: _loading,
          onRun: () => _run('Decrypt', () async {
            final sink = _MemorySink();
            await _pdf.decrypt(_MemorySource(bytes), sink, password: 'secret123');
            await _saveAndReport(sink.takeBytes(), 'decrypted.pdf');
          })),
        const SizedBox(height: 12),

        // ── TEXT EXTRACTION ──
        _SectionLabel('Text Extraction'),
        _OpTile(icon: Icons.text_snippet, title: 'Extract all text',
          subtitle: 'Plain text from every page', loading: _loading,
          onRun: () => _run('Extract text', () async {
            final t = await _pdf.extract(_MemorySource(bytes), pages: const PdfPages.all());
            if (!mounted) return;
            _showTextSheet(context, 'Extracted Text (${t.length} chars)', t);
            setState(() => _status = '${t.length} characters extracted');
          })),
        _OpTile(icon: Icons.text_snippet_outlined, title: 'Extract page 1 text',
          subtitle: 'First page only', loading: _loading,
          onRun: () => _run('Extract page 1', () async {
            final t = await _pdf.extract(_MemorySource(bytes), pages: const PdfPages.single(0));
            if (!mounted) return;
            _showTextSheet(context, 'Page 1 Text', t);
            setState(() => _status = 'Page 1: ${t.length} chars');
          })),
        _OpTile(icon: Icons.code, title: 'To Markdown',
          subtitle: 'Markdown from all pages', loading: _loading,
          onRun: () => _run('To Markdown', () async {
            final md = await _pdf.extract(_MemorySource(bytes), pages: const PdfPages.all(),
                format: PdfExtractionFormat.markdown);
            if (!mounted) return;
            _showTextSheet(context, 'Markdown', md);
            setState(() => _status = 'Markdown: ${md.length} chars');
          })),
        _OpTile(icon: Icons.html, title: 'To HTML (page 1)',
          subtitle: 'HTML from first page', loading: _loading,
          onRun: () => _run('To HTML', () async {
            final h = await _pdf.extract(_MemorySource(bytes), pages: const PdfPages.single(0),
                format: PdfExtractionFormat.html);
            if (!mounted) return;
            _showTextSheet(context, 'HTML', h);
            setState(() => _status = 'HTML: ${h.length} chars');
          })),
        const SizedBox(height: 12),

        // ── SEARCH ──
        _SectionLabel('Search'),
        _OpTile(icon: Icons.search, title: 'Search page 1 for "the"',
          subtitle: 'Find text positions on first page', loading: _loading,
          onRun: () => _run('Search', () async {
            final r = await _pdf.search(_MemorySource(bytes), query: 'the', pages: const PdfPages.single(0));
            setState(() => _status = '${r.length} results on page 1');
          })),
        _OpTile(icon: Icons.manage_search, title: 'Search all pages for "the"',
          subtitle: 'Find across entire document', loading: _loading,
          onRun: () => _run('Search all', () async {
            final r = await _pdf.search(_MemorySource(bytes), query: 'the', pages: const PdfPages.all());
            setState(() => _status = '${r.length} results across all pages');
          })),
        const SizedBox(height: 12),

        // ── FORMS & ANNOTATIONS ──
        _SectionLabel('Forms & Annotations'),
        _OpTile(icon: Icons.layers_clear, title: 'Flatten forms',
          subtitle: 'Burn form fields into page content', loading: _loading,
          onRun: () => _run('Flatten forms', () async {
            final sink = _MemorySink();
            await _pdf.flattenForms(_MemorySource(bytes), sink);
            await _saveAndReport(sink.takeBytes(), 'flattened_forms.pdf');
          })),
        _OpTile(icon: Icons.rule, title: 'Apply redactions',
          subtitle: 'Apply pending redaction marks', loading: _loading,
          onRun: () => _run('Apply redactions', () async {
            final sink = _MemorySink();
            await _pdf.applyRedactions(_MemorySource(bytes), sink);
            await _saveAndReport(sink.takeBytes(), 'redacted.pdf');
          })),
        const SizedBox(height: 12),

        // ── PROBE ──
        _SectionLabel('Validate'),
        _OpTile(icon: Icons.verified, title: 'Quick probe',
          subtitle: 'Validate without full parse', loading: _loading,
          onRun: () => _run('Probe', () async {
            final doc = await _pdf.open(_MemorySource(bytes));
            setState(() => _status =
                '${doc.pageCount} pages • '
                'Encrypted: ${doc.isEncrypted} • v${doc.version}');
          })),

        const SizedBox(height: 40),
      ],
    );
  }

  void _showTextSheet(BuildContext context, String title, String text) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (ctx) => DraggableScrollableSheet(
        expand: false,
        initialChildSize: 0.7,
        maxChildSize: 0.95,
        builder: (ctx, scrollCtrl) => Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 8, 8),
              child: Row(
                children: [
                  Expanded(child: Text(title, style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 16))),
                  IconButton(icon: const Icon(Icons.close), onPressed: () => Navigator.pop(ctx)),
                ],
              ),
            ),
            const Divider(height: 1),
            Expanded(
              child: SingleChildScrollView(
                controller: scrollCtrl,
                padding: const EdgeInsets.all(16),
                child: SelectableText(
                  text.isEmpty ? '(no text found)' : text,
                  style: const TextStyle(fontSize: 13, fontFamily: 'monospace', height: 1.5),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ─── Tab 2: Merge ──────────────────────────────────────────────────

class _MergeTab extends StatefulWidget {
  const _MergeTab();
  @override
  State<_MergeTab> createState() => _MergeTabState();
}

class _MergeTabState extends State<_MergeTab> {
  final _pdf = Pdf();

  final List<PickedFile> _files = [];
  Uint8List? _merged;
  bool _loading = false;
  String? _status;

  @override
  void dispose() {
    _pdf.dispose();
    super.dispose();
  }

  Future<void> _addFiles() async {
    final result = await FilePicker.platform.pickFiles(
      allowMultiple: true,
      type: FileType.custom,
      allowedExtensions: ['pdf'],
      withData: true,
    );
    if (result == null) return;
    for (final f in result.files) {
      if (f.bytes != null) {
        _files.add(PickedFile(name: f.name, bytes: f.bytes!, size: f.size));
      }
    }
    setState(() { _merged = null; _status = '${_files.length} files ready'; });
  }

  Future<void> _merge() async {
    if (_files.length < 2) return;
    setState(() { _loading = true; _status = 'Merging...'; });
    try {
      final sources = _files.map((f) => _MemorySource(f.bytes) as PdfSource).toList();
      final sink = _MemorySink();
      await _pdf.merge(sources, sink);
      final r = sink.takeBytes();
      setState(() { _merged = r; _status = 'Merged: ${fmtSize(r.length)}'; });
    } on PdfError catch (e) {
      setState(() => _status = 'Error: $e');
    } finally {
      setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Merge PDFs')),
      body: Column(
        children: [
          _StatusBar(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
          Expanded(
            child: _files.isEmpty
                ? Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.merge, size: 56, color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
                        const SizedBox(height: 16),
                        const Text('Pick 2 or more PDFs to merge'),
                        const SizedBox(height: 16),
                        FilledButton.icon(onPressed: _addFiles, icon: const Icon(Icons.add), label: const Text('Pick PDFs')),
                      ],
                    ),
                  )
                : ReorderableListView.builder(
                    padding: const EdgeInsets.all(16),
                    itemCount: _files.length,
                    onReorder: (old, to) {
                      setState(() {
                        if (to > old) to--;
                        final item = _files.removeAt(old);
                        _files.insert(to, item);
                        _merged = null;
                      });
                    },
                    itemBuilder: (_, i) => Card(
                      key: ValueKey('${_files[i].name}_$i'),
                      margin: const EdgeInsets.symmetric(vertical: 3),
                      child: ListTile(
                        leading: CircleAvatar(radius: 16, child: Text('${i + 1}', style: const TextStyle(fontSize: 13))),
                        title: Text(_files[i].name, style: const TextStyle(fontSize: 14)),
                        subtitle: Text(fmtSize(_files[i].size), style: const TextStyle(fontSize: 12)),
                        trailing: IconButton(icon: const Icon(Icons.close, size: 18),
                            onPressed: () => setState(() { _files.removeAt(i); _merged = null; })),
                        dense: true,
                      ),
                    ),
                  ),
          ),
          if (_files.isNotEmpty)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  OutlinedButton.icon(onPressed: _loading ? null : _addFiles,
                      icon: const Icon(Icons.add, size: 18), label: const Text('Add')),
                  const SizedBox(width: 8),
                  Expanded(
                    child: _merged != null
                        ? FilledButton.icon(
                            onPressed: () async {
                              final p = await saveBytes(_merged!, 'merged.pdf');
                              if (p != null) setState(() => _status = 'Saved: ${p.split('/').last}');
                            },
                            icon: const Icon(Icons.save, size: 18), label: const Text('Save'))
                        : FilledButton.icon(
                            onPressed: _files.length >= 2 && !_loading ? _merge : null,
                            icon: _loading
                                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white))
                                : const Icon(Icons.merge, size: 18),
                            label: Text('Merge ${_files.length}')),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

// ─── Tab 3: Images → PDF ───────────────────────────────────────────

class _ImagesToPdfTab extends StatefulWidget {
  const _ImagesToPdfTab();
  @override
  State<_ImagesToPdfTab> createState() => _ImagesToPdfTabState();
}

class _ImagesToPdfTabState extends State<_ImagesToPdfTab> {
  final _pdf = Pdf();

  final List<PickedFile> _images = [];
  Uint8List? _pdfBytes;
  bool _loading = false;
  String? _status;

  @override
  void dispose() {
    _pdf.dispose();
    super.dispose();
  }

  Future<void> _addImages() async {
    final result = await pickImageBytes();
    if (result == null) return;
    _images.addAll(result);
    setState(() { _pdfBytes = null; _status = '${_images.length} images ready'; });
  }

  Future<void> _convert() async {
    if (_images.isEmpty) return;
    setState(() { _loading = true; _status = 'Converting...'; });
    try {
      final imgs = _images.map((f) => f.bytes).toList();
      final sink = _MemorySink();
      await _pdf.imagesToPdf(imgs, sink);
      final r = sink.takeBytes();
      setState(() { _pdfBytes = r; _status = 'Created: ${fmtSize(r.length)}'; });
    } on PdfError catch (e) {
      setState(() => _status = 'Error: $e');
    } finally {
      setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Images → PDF')),
      body: Column(
        children: [
          _StatusBar(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
          Expanded(
            child: _images.isEmpty
                ? Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.add_photo_alternate, size: 56,
                            color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
                        const SizedBox(height: 16),
                        const Text('Pick images to convert'),
                        const SizedBox(height: 16),
                        FilledButton.icon(onPressed: _addImages,
                            icon: const Icon(Icons.add_photo_alternate), label: const Text('Pick Images')),
                      ],
                    ),
                  )
                : ReorderableListView.builder(
                    padding: const EdgeInsets.all(16),
                    itemCount: _images.length,
                    onReorder: (old, to) {
                      setState(() {
                        if (to > old) to--;
                        final item = _images.removeAt(old);
                        _images.insert(to, item);
                        _pdfBytes = null;
                      });
                    },
                    itemBuilder: (_, i) => Card(
                      key: ValueKey('${_images[i].name}_$i'),
                      margin: const EdgeInsets.symmetric(vertical: 3),
                      child: ListTile(
                        leading: CircleAvatar(radius: 16, child: Text('${i + 1}', style: const TextStyle(fontSize: 13))),
                        title: Text(_images[i].name, style: const TextStyle(fontSize: 14)),
                        subtitle: Text(fmtSize(_images[i].size)),
                        trailing: IconButton(icon: const Icon(Icons.close, size: 18),
                            onPressed: () => setState(() { _images.removeAt(i); _pdfBytes = null; })),
                        dense: true,
                      ),
                    ),
                  ),
          ),
          if (_images.isNotEmpty)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  OutlinedButton.icon(onPressed: _loading ? null : _addImages,
                      icon: const Icon(Icons.add, size: 18), label: const Text('Add')),
                  const SizedBox(width: 8),
                  Expanded(
                    child: _pdfBytes != null
                        ? FilledButton.icon(
                            onPressed: () async {
                              final p = await saveBytes(_pdfBytes!, 'from_images.pdf');
                              if (p != null) setState(() => _status = 'Saved: ${p.split('/').last}');
                            },
                            icon: const Icon(Icons.save, size: 18), label: const Text('Save'))
                        : FilledButton.icon(
                            onPressed: _images.isNotEmpty && !_loading ? _convert : null,
                            icon: _loading
                                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white))
                                : const Icon(Icons.picture_as_pdf, size: 18),
                            label: Text('Convert ${_images.length}')),
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

// ─── Tab 4: PdfEditor — batch mutations ────────────────────────────

class _EditorTab extends StatefulWidget {
  const _EditorTab();
  @override
  State<_EditorTab> createState() => _EditorTabState();
}

class _EditorTabState extends State<_EditorTab> {
  final _pdf = Pdf();

  Uint8List? _fileBytes;
  String? _fileName;
  bool _loading = false;
  String? _status;

  @override
  void dispose() {
    _pdf.dispose();
    super.dispose();
  }

  Future<void> _pickPdf() async {
    final result = await pickPdfBytes();
    if (result == null) return;
    final bytes = result.first;
    setState(() {
      _fileBytes = bytes;
      _fileName = 'Selected PDF';
      _status = 'Loaded $_fileName';
    });
  }

  Future<void> _runEditor(String label, Future<Uint8List> Function(PdfEditor editor) work) async {
    if (_fileBytes == null) return;
    setState(() { _loading = true; _status = '$label...'; });
    try {
      final editor = await _pdf.edit(_MemorySource(_fileBytes!));
      try {
        final result = await work(editor);
        final path = await saveBytes(result, '${label.toLowerCase().replaceAll(' ', '_')}.pdf');
        setState(() => _status = path != null ? 'Saved (${fmtSize(result.length)})' : 'Cancelled');
      } finally {
        editor.dispose();
      }
    } on PdfError catch (e) {
      setState(() => _status = 'Error: $e');
    } catch (e) {
      setState(() => _status = 'Error: $e');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('PdfEditor')),
      body: Column(
        children: [
          _StatusBar(loading: _loading, message: _status, onDismiss: () => setState(() => _status = null)),
          Expanded(
            child: _fileBytes == null
                ? Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.edit_document, size: 56,
                            color: Theme.of(context).colorScheme.primary.withValues(alpha: 0.5)),
                        const SizedBox(height: 16),
                        const Text('Editor: parse once, mutate N times, save once'),
                        const SizedBox(height: 6),
                        Text('More efficient than one-shot Pdf methods for batch operations',
                            style: TextStyle(fontSize: 12, color: Theme.of(context).colorScheme.onSurfaceVariant)),
                        const SizedBox(height: 20),
                        FilledButton.icon(onPressed: _pickPdf,
                            icon: const Icon(Icons.file_open), label: const Text('Open PDF')),
                      ],
                    ),
                  )
                : ListView(
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                    children: [
                      Card(
                        child: ListTile(
                          leading: Icon(Icons.picture_as_pdf, color: Theme.of(context).colorScheme.primary),
                          title: Text(_fileName ?? '', style: const TextStyle(fontWeight: FontWeight.w600, fontSize: 14)),
                          subtitle: Text(fmtSize(_fileBytes!.length)),
                          trailing: IconButton.filledTonal(onPressed: _pickPdf,
                              icon: const Icon(Icons.swap_horiz, size: 20)),
                          dense: true,
                        ),
                      ),
                      const SizedBox(height: 8),

                      _SectionLabel('Metadata'),
                      _OpTile(icon: Icons.title, title: 'Set title & author', loading: _loading,
                        subtitle: 'Title: "Test PDF", Author: "pdf_manipulator"',
                        onRun: () => _runEditor('Set metadata', (e) async {
                          await e.setTitle('Test PDF');
                          await e.setAuthor('pdf_manipulator');
                          await e.setSubject('Example output');
                          final sink = _MemorySink();
                          await e.save(sink);
                          return sink.takeBytes();
                        })),

                      _SectionLabel('Page Operations'),
                      _OpTile(icon: Icons.rotate_right, title: 'Rotate page 1 → 90°', loading: _loading,
                        onRun: () => _runEditor('Rotate page 1', (e) async {
                          await e.rotatePage(0, degrees: 90);
                          final sink = _MemorySink();
                          await e.save(sink);
                          return sink.takeBytes();
                        })),
                      _OpTile(icon: Icons.rotate_right, title: 'Rotate all → 90°', loading: _loading,
                        onRun: () => _runEditor('Rotate all', (e) async {
                          await e.rotateAllPages(degrees: 90);
                          final sink = _MemorySink();
                          await e.save(sink);
                          return sink.takeBytes();
                        })),
                      _OpTile(icon: Icons.delete_outline, title: 'Delete last page', loading: _loading,
                        onRun: () => _runEditor('Delete last page', (e) async {
                          if (await e.pageCount > 1) await e.deletePage(await e.pageCount - 1);
                          final sink = _MemorySink();
                          await e.save(sink);
                          return sink.takeBytes();
                        })),

                      _SectionLabel('Watermark & Compress'),
                      _OpTile(icon: Icons.water_drop, title: 'Watermark all pages "SAMPLE"', loading: _loading,
                        onRun: () => _runEditor('Watermark', (e) async {
                          final pc = await e.pageCount;
                          for (var i = 0; i < pc; i++) {
                            await e.addWatermark(i, 'SAMPLE',
                                style: const PdfWatermarkStyle(opacity: 0.25, fontSize: 48));
                          }
                          final sink = _MemorySink();
                          await e.save(sink);
                          return sink.takeBytes();
                        })),
                      _OpTile(icon: Icons.compress, title: 'Optimize images (quality 60)', loading: _loading,
                        onRun: () => _runEditor('Optimize images', (e) async {
                          final count = await e.optimizeImages(quality: 60);
                          log('Optimized $count images');
                          final sink = _MemorySink();
                          await e.save(sink, options: const PdfSaveOptions(compress: true, garbageCollect: true));
                          return sink.takeBytes();
                        })),

                      _SectionLabel('Forms'),
                      _OpTile(icon: Icons.layers_clear, title: 'Flatten forms', loading: _loading,
                        onRun: () => _runEditor('Flatten forms', (e) async {
                          await e.flattenForms();
                          final sink = _MemorySink();
                          await e.save(sink);
                          return sink.takeBytes();
                        })),
                      _OpTile(icon: Icons.layers_clear_outlined, title: 'Flatten all annotations', loading: _loading,
                        onRun: () => _runEditor('Flatten annotations', (e) async {
                          await e.flattenAllAnnotations();
                          final sink = _MemorySink();
                          await e.save(sink);
                          return sink.takeBytes();
                        })),

                      _SectionLabel('Security'),
                      _OpTile(icon: Icons.lock, title: 'Save encrypted (pw: test123)', loading: _loading,
                        onRun: () => _runEditor('Encrypt', (e) async {
                          final sink = _MemorySink();
                          await e.save(sink, options: const PdfSaveOptions(
                              encryption: PdfEncryptionConfig(ownerPassword: 'test123')));
                          return sink.takeBytes();
                        })),

                      _SectionLabel('Chained Operations'),
                      _OpTile(icon: Icons.auto_fix_high, title: 'Rotate + Watermark + Compress', loading: _loading,
                        subtitle: 'Demonstrates batch mutations in one pass',
                        onRun: () => _runEditor('Chain operations', (e) async {
                          await e.rotateAllPages(degrees: 90);
                          final pc = await e.pageCount;
                          for (var i = 0; i < pc; i++) {
                            await e.addWatermark(i, 'PROCESSED',
                                style: const PdfWatermarkStyle(opacity: 0.15, fontSize: 40));
                          }
                          await e.optimizeImages(quality: 70);
                          await e.setTitle('Processed by pdf_manipulator');
                          final sink = _MemorySink();
                          await e.save(sink, options: const PdfSaveOptions(compress: true, garbageCollect: true));
                          return sink.takeBytes();
                        })),

                      const SizedBox(height: 40),
                    ],
                  ),
          ),
        ],
      ),
    );
  }
}

// ─── Shared widgets ────────────────────────────────────────────────

class _SectionLabel extends StatelessWidget {
  final String text;
  const _SectionLabel(this.text);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 12, bottom: 4, left: 4),
      child: Text(
        text.toUpperCase(),
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w700,
          letterSpacing: 1.2,
          color: Theme.of(context).colorScheme.primary,
        ),
      ),
    );
  }
}

class _OpTile extends StatelessWidget {
  final IconData icon;
  final String title;
  final String? subtitle;
  final VoidCallback onRun;
  final bool loading;

  const _OpTile({
    required this.icon,
    required this.title,
    this.subtitle,
    required this.onRun,
    this.loading = false,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 2),
      child: ListTile(
        leading: Icon(icon, size: 22),
        title: Text(title, style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500)),
        subtitle: subtitle != null ? Text(subtitle!, style: const TextStyle(fontSize: 12)) : null,
        trailing: SizedBox(
          width: 64,
          height: 34,
          child: FilledButton.tonal(
            onPressed: loading ? null : onRun,
            style: FilledButton.styleFrom(padding: EdgeInsets.zero, textStyle: const TextStyle(fontSize: 13)),
            child: const Text('Run'),
          ),
        ),
        dense: true,
        visualDensity: VisualDensity.compact,
      ),
    );
  }
}

class _ExpandableSection extends StatefulWidget {
  final IconData icon;
  final String title;
  final Widget child;
  const _ExpandableSection({required this.icon, required this.title, required this.child});

  @override
  State<_ExpandableSection> createState() => _ExpandableSectionState();
}

class _ExpandableSectionState extends State<_ExpandableSection> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(vertical: 2),
      child: Column(
        children: [
          ListTile(
            leading: Icon(widget.icon, size: 22),
            title: Text(widget.title, style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500)),
            trailing: Icon(_expanded ? Icons.expand_less : Icons.expand_more),
            onTap: () => setState(() => _expanded = !_expanded),
            dense: true,
          ),
          if (_expanded) ...[
            const Divider(height: 1),
            Padding(padding: const EdgeInsets.symmetric(vertical: 8), child: widget.child),
          ],
        ],
      ),
    );
  }
}

class PickedFile {
  final String name;
  final Uint8List bytes;
  final int size;
  const PickedFile({required this.name, required this.bytes, required this.size});
}
