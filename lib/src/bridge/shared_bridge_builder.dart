// Builder + page-builder handles — create from scratch.
//
// Part of shared_bridge.dart — same library so the handles reach the
// bridge's private _exec/_execKeepSource (where every PdfTask is
// born). One handle concern per file; zero logic beyond
// encode → _exec → decode.

part of 'shared_bridge.dart';

// ══════════════════════════════════════════════════════════════════
// Builder handle
// ══════════════════════════════════════════════════════════════════

class _SharedBuilderHandle extends BridgeBuilderHandle {
  _SharedBuilderHandle(this._bridge, this._handleId);
  final SharedBridge _bridge;
  final int _handleId;

  PdfTask<Map<String, Object?>> _exec(EngineOp op, Map<String, Object?> args) =>
      _bridge._exec(op, {'handleId': _handleId, ...args});

  @override
  PdfTask<void> setTitle(String value) =>
      _exec(EngineOp.builderSetMetadata, {'title': value});
  @override
  PdfTask<void> setAuthor(String value) =>
      _exec(EngineOp.builderSetMetadata, {'author': value});
  @override
  PdfTask<void> setSubject(String value) =>
      _exec(EngineOp.builderSetMetadata, {'subject': value});
  @override
  PdfTask<void> setKeywords(String value) =>
      _exec(EngineOp.builderSetMetadata, {'keywords': value});

  @override
  PdfTask<BridgePageBuilderHandle> addA4Page() => _exec(
    EngineOp.builderAddPage,
    {'pageType': 'a4'},
  ).map((_) => _SharedPageBuilderHandle(_bridge, _handleId));
  @override
  PdfTask<BridgePageBuilderHandle> addLetterPage() => _exec(
    EngineOp.builderAddPage,
    {'pageType': 'letter'},
  ).map((_) => _SharedPageBuilderHandle(_bridge, _handleId));
  @override
  PdfTask<BridgePageBuilderHandle> addPage({
    required double width,
    required double height,
  }) => _exec(EngineOp.builderAddPage, {
    'width': width,
    'height': height,
  }).map((_) => _SharedPageBuilderHandle(_bridge, _handleId));

  @override
  PdfTask<void> save(DataSink output) => _bridge._exec(
    EngineOp.builderSave,
    {'handleId': _handleId},
    sinks: [output],
  );
  @override
  Future<void> dispose() => _exec(EngineOp.builderDispose, {});
}

// ══════════════════════════════════════════════════════════════════
// Page builder handle
// ══════════════════════════════════════════════════════════════════

class _SharedPageBuilderHandle extends BridgePageBuilderHandle {
  _SharedPageBuilderHandle(this._bridge, this._handleId);
  final SharedBridge _bridge;
  final int _handleId;

  PdfTask<void> _op(String op, [Map<String, Object?> extra = const {}]) =>
      _bridge._exec(EngineOp.builderPageOp, {
        'handleId': _handleId,
        'pageOp': op,
        ...extra,
      });

  @override
  PdfTask<void> font(String name, double size) =>
      _op('font', {'name': name, 'size': size});
  @override
  PdfTask<void> at(double x, double y) => _op('at', {'x': x, 'y': y});
  @override
  PdfTask<void> text(String text) => _op('text', {'text': text});
  @override
  PdfTask<void> heading(int level, String text) =>
      _op('heading', {'level': level, 'text': text});
  @override
  PdfTask<void> paragraph(String text) => _op('paragraph', {'text': text});
  @override
  PdfTask<void> space(double points) => _op('space', {'points': points});
  @override
  PdfTask<void> horizontalRule() => _op('horizontalRule');
  @override
  PdfTask<void> image(
    DataSource imageData,
    PdfRect rect, {
    String altText = '',
  }) => _bridge._exec(
    EngineOp.builderPageOp,
    {
      'handleId': _handleId,
      'pageOp': 'image',
      'sourceLength': imageData.length,
      ...codec.encodeRectArgs(rect),
      'altText': altText,
    },
    sources: [imageData],
  );
  @override
  PdfTask<void> watermark(String text) => _op('watermark', {'text': text});
  @override
  PdfTask<void> textField(String name, PdfRect rect, {String? defaultValue}) =>
      _op('textField', {
        'name': name,
        ...codec.encodeRectArgs(rect),
        'defaultValue': defaultValue,
      });
  @override
  PdfTask<void> checkbox(String name, PdfRect rect, {bool checked = false}) =>
      _op('checkbox', {
        'name': name,
        ...codec.encodeRectArgs(rect),
        'checked': checked,
      });
  @override
  PdfTask<void> comboBox(
    String name,
    PdfRect rect,
    List<String> options, {
    String? selected,
  }) => _op('comboBox', {
    'name': name,
    ...codec.encodeRectArgs(rect),
    'options': options,
    'selected': selected,
  });
  @override
  PdfTask<void> pushButton(String name, PdfRect rect, String caption) => _op(
    'pushButton',
    {'name': name, ...codec.encodeRectArgs(rect), 'caption': caption},
  );
  @override
  PdfTask<void> signatureField(String name, PdfRect rect) =>
      _op('signatureField', {'name': name, ...codec.encodeRectArgs(rect)});
  @override
  PdfTask<void> radioGroup(
    String name,
    List<({String value, PdfRect rect})> options, {
    String? selected,
  }) => _op('radioGroup', {
    'name': name,
    'options': options
        .map(
          (o) => <String, Object?>{
            'value': o.value,
            ...codec.encodeRectArgs(o.rect),
          },
        )
        .toList(),
    'selected': selected,
  });
  @override
  PdfTask<void> fieldKeystroke(String script) =>
      _op('fieldKeystroke', {'script': script});
  @override
  PdfTask<void> fieldFormat(String script) =>
      _op('fieldFormat', {'script': script});
  @override
  PdfTask<void> fieldValidate(String script) =>
      _op('fieldValidate', {'script': script});
  @override
  PdfTask<void> fieldCalculate(String script) =>
      _op('fieldCalculate', {'script': script});
  @override
  PdfTask<void> linkUrl(String url) => _op('linkUrl', {'url': url});
  @override
  PdfTask<void> linkPage(int targetPage) =>
      _op('linkPage', {'targetPage': targetPage});
  @override
  PdfTask<void> footnote(String refMark, String noteText) =>
      _op('footnote', {'refMark': refMark, 'noteText': noteText});
  @override
  PdfTask<void> columns(int columnCount, double gapPt, String text) => _op(
    'columns',
    {'columnCount': columnCount, 'gapPt': gapPt, 'text': text},
  );
  @override
  PdfTask<void> newline() => _op('newline');
  @override
  PdfTask<void> newPageSameSize() => _op('newPageSameSize');
  @override
  PdfTask<void> done() =>
      _bridge._exec(EngineOp.builderPageDone, {'handleId': _handleId});
}
