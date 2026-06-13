// Dependency-constraint report.
//
// The doctrine it reports against:
//   - Regular dependencies keep LOW floors — the oldest version the
//     code works with. Pub resolves the newest allowed anyway; a high
//     floor only shrinks the set of consumers that can install this
//     package. A newer version beyond the current resolution means
//     "verify compatibility, then widen the range" — human judgment.
//   - Dev dependencies are invisible to consumers — their floors track
//     the latest version this package develops against.
//
// Always exits 0: dependency drift is advisory, never a build break.
// Under GitHub Actions, findings surface as ::warning:: annotations —
// the run stays green and carries yellow flags.
//
// Run via `make check-deps`.

import 'dart:convert';
import 'dart:io';

Future<void> main() async {
  final result = await Process.run(Platform.resolvedExecutable, [
    'pub',
    'outdated',
    '--json',
  ]);
  if (result.exitCode != 0) {
    stderr.write(result.stderr);
    exit(result.exitCode); // could not even query — that IS a failure
  }

  final report = jsonDecode(result.stdout as String) as Map<String, dynamic>;
  final packages = (report['packages'] as List).cast<Map<String, dynamic>>();

  String? versionOf(Map<String, dynamic> p, String key) =>
      (p[key] as Map<String, dynamic>?)?['version'] as String?;

  final inCi = Platform.environment['GITHUB_ACTIONS'] == 'true';
  void warn(String message) {
    stdout.writeln(inCi ? '::warning::$message' : 'WARN  $message');
  }

  var findings = 0;
  for (final p in packages) {
    final kind = p['kind'] as String;
    if (kind == 'transitive') continue;

    final name = p['package'] as String;
    final current = versionOf(p, 'current');
    final resolvable = versionOf(p, 'resolvable');
    final latest = versionOf(p, 'latest');
    if (latest == null || current == latest) continue;
    findings++;

    if (kind == 'dev') {
      if (resolvable == latest) {
        warn(
          'dev dep $name: floor allows $current, latest is $latest — '
          'set ^$latest in pubspec and run `dart pub upgrade`.',
        );
      } else {
        warn(
          'dev dep $name: latest $latest is held at $resolvable by a '
          'peer constraint — nothing to do until the peer unpins.',
        );
      }
    } else {
      warn(
        'dependency $name: $latest exists beyond the current resolution '
        '($current). Floors stay low — if our upper bound is the blocker, '
        'verify compatibility and widen the range; if a peer constraint '
        'holds it, nothing to do until the peer unpins.',
      );
    }
  }

  stdout.writeln(
    findings == 0
        ? 'deps: everything current.'
        : 'deps: $findings advisory finding(s) — see warnings above.',
  );
}
