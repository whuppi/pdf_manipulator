// PDF date strings (ISO 32000-1 §7.9.4): D:YYYYMMDDHHmmSSOHH'mm'
// Every part after the year is optional.

final _pdfDate = RegExp(
  r"^(?:D:)?(\d{4})(\d{2})?(\d{2})?(\d{2})?(\d{2})?(\d{2})?"
  r"(?:(Z)|([+-])(\d{2})'?(?:(\d{2})'?)?)?$",
);

/// Parses a PDF date string to a UTC [DateTime]. A string that does not
/// parse, or names a moment that does not exist, gives `null`. A date with
/// no offset is read as UTC.
DateTime? parsePdfDate(String? value) {
  if (value == null) return null;
  final m = _pdfDate.firstMatch(value.trim());
  if (m == null) return null;
  int part(int group, int fallback) =>
      m.group(group) == null ? fallback : int.parse(m.group(group)!);
  final year = int.parse(m.group(1)!);
  final month = part(2, 1);
  final day = part(3, 1);
  final hour = part(4, 0);
  final minute = part(5, 0);
  final second = part(6, 0);
  final offsetHours = part(9, 0);
  final offsetMinutes = part(10, 0);
  if (month < 1 || month > 12 || day < 1) return null;
  if (hour > 23 || minute > 59 || second > 59) return null;
  if (offsetHours > 23 || offsetMinutes > 59) return null;
  final local = DateTime.utc(year, month, day, hour, minute, second);
  if (local.month != month || local.day != day) return null;
  final offset = Duration(hours: offsetHours, minutes: offsetMinutes);
  return m.group(8) == '-' ? local.add(offset) : local.subtract(offset);
}

/// Formats [value] as a PDF date string in UTC: `D:YYYYMMDDHHmmSSZ`.
String formatPdfDate(DateTime value) {
  final u = value.toUtc();
  String two(int n) => n.toString().padLeft(2, '0');
  return 'D:${u.year.toString().padLeft(4, '0')}${two(u.month)}${two(u.day)}'
      '${two(u.hour)}${two(u.minute)}${two(u.second)}Z';
}
