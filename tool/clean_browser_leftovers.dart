// clean_browser_leftovers.dart — removes what killed headless browser
// launches leave behind, on any OS.
//
//   dart tool/clean_browser_leftovers.dart
//
// The browser tests launch Chrome many times a run, and a launch that is
// killed (a timeout, a deadline, an interrupted run) leaves its state behind:
//
//   - on every OS, its profile folder `org.chromium.Chromium.scoped_dir.*` in
//     the system temp dir;
//   - on macOS also a copy of the whole app bundle in
//     `$(getconf DARWIN_USER_TEMP_DIR)/../X/com.google.Chrome.code_sign_clone/`,
//     which macOS makes at every launch of a signed app (over a gigabyte each).
//
// Both belong to a running Chrome or to none, so the tool removes them only
// when no Chrome process runs at all, and then every one is a leftover.
// Prints how many it removed. Always exits 0: a cleanup that fails must not
// fail the test that called it.

import 'dart:io';

Future<void> main() async {
  try {
    if (await _chromeRunning()) {
      stdout.writeln('browser leftovers: a Chrome is running, none removed');
      return;
    }
    var removed = 0;
    removed += _removeMatching(
      Directory.systemTemp,
      'org.chromium.Chromium.scoped_dir.',
    );
    final clones = await _macCloneDir();
    if (clones != null) removed += _removeMatching(clones, 'code_sign_clone.');
    stdout.writeln('browser leftovers: removed $removed');
  } catch (e) {
    stdout.writeln('browser leftovers: skipped ($e)');
  }
}

/// Every Chrome flavour a test can launch: the browser and its helpers,
/// Chrome for Testing, Chromium, the headless shell and chromedriver. Matched
/// on the process name, never the command line, so an Electron app's
/// chrome_crashpad_handler and this tool's own command do not count.
final RegExp _chromeName = RegExp(
  r'^(google chrome.*|chromium.*|chromedriver(\.exe)?|chrome(\.exe)?|chrome-headless-shell(\.exe)?)$',
  caseSensitive: false,
);

Future<bool> _chromeRunning() async {
  final List<String> names;
  if (Platform.isWindows) {
    final r = await Process.run('tasklist', ['/fo', 'csv', '/nh']);
    names = [
      for (final line in (r.stdout as String).split('\n'))
        if (line.startsWith('"')) line.substring(1, line.indexOf('"', 1)),
    ];
  } else {
    // `ps -A -o comm=` gives each process's name (its executable's path on
    // macOS); the name is its last path segment.
    final r = await Process.run('ps', ['-A', '-o', 'comm=']);
    names = [
      for (final line in (r.stdout as String).split('\n'))
        if (line.trim().isNotEmpty) line.trim().split('/').last,
    ];
  }
  return names.any(_chromeName.hasMatch);
}

/// macOS's per-user clone folder for Chrome, or null on any other OS or when
/// it does not exist.
Future<Directory?> _macCloneDir() async {
  if (!Platform.isMacOS) return null;
  final r = await Process.run('getconf', ['DARWIN_USER_TEMP_DIR']);
  final userTemp = (r.stdout as String).trim();
  if (userTemp.isEmpty) return null;
  final dir = Directory('$userTemp../X/com.google.Chrome.code_sign_clone');
  return dir.existsSync() ? dir : null;
}

int _removeMatching(Directory parent, String prefix) {
  if (!parent.existsSync()) return 0;
  var removed = 0;
  for (final entry in parent.listSync(followLinks: false)) {
    final name = entry.uri.pathSegments.where((s) => s.isNotEmpty).last;
    if (entry is Directory && name.startsWith(prefix)) {
      try {
        entry.deleteSync(recursive: true);
        removed++;
      } on FileSystemException {
        // A folder the OS still holds stays for the next run.
      }
    }
  }
  return removed;
}
