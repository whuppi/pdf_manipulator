// Text scanning shared by the API walls. The walls read source as text, the
// way the other source-scan guards do: the package's runtime code never
// depends on `package:analyzer`, and a test does not need a resolved tree to
// see a call site or a declaration.

/// [source] with `//` comments (and so `///` doc comments) blanked out,
/// keeping every offset and line break where they were.
String stripComments(String source) {
  final out = StringBuffer();
  for (final line in source.split('\n')) {
    final at = _commentStart(line);
    out.writeln(at < 0 ? line : line.substring(0, at).padRight(line.length));
  }
  final text = out.toString();
  return text.substring(0, text.length - 1);
}

/// Where a `//` comment starts on [line], outside any string literal, or -1.
int _commentStart(String line) {
  String? quote;
  for (var i = 0; i < line.length; i++) {
    final c = line[i];
    if (quote != null) {
      if (c == r'\') {
        i++;
      } else if (c == quote) {
        quote = null;
      }
    } else if (c == "'" || c == '"') {
      quote = c;
    } else if (c == '/' && i + 1 < line.length && line[i + 1] == '/') {
      return i;
    }
  }
  return -1;
}

/// The index of the bracket that closes the one opened at [openAt].
int matchingClose(String text, int openAt) {
  const pairs = {'(': ')', '{': '}', '[': ']'};
  final open = text[openAt];
  final close = pairs[open]!;
  var depth = 0;
  for (var i = openAt; i < text.length; i++) {
    if (text[i] == open) depth++;
    if (text[i] == close && --depth == 0) return i;
  }
  throw FormatException('no closing $close for the $open at $openAt');
}

/// 1-based line of [offset] in [text].
int lineOf(String text, int offset) =>
    '\n'.allMatches(text.substring(0, offset)).length + 1;
