// Generate a <details> commit list for a changelog entry.
//
// Usage:
//   dart run tool/commits.dart v1.0.0          — commits since tag v1.0.0
//   dart run tool/commits.dart v1.1.0-dev.0    — commits since that prerelease tag
//   dart run tool/commits.dart abc1234         — commits since any git ref
//
// Output goes to stdout (copy it). Status goes to stderr.
import 'dart:io';

void main(List<String> args) {
  if (args.isEmpty) {
    stderr.writeln('Usage: dart run tool/commits.dart <git-ref>');
    stderr.writeln('');
    stderr.writeln('Examples:');
    stderr.writeln('  dart run tool/commits.dart v1.0.0');
    stderr.writeln('  dart run tool/commits.dart v1.1.0-dev.0');
    stderr.writeln('  dart run tool/commits.dart abc1234');
    exit(1);
  }

  final ref = args.first;

  // Verify ref exists
  final check = Process.runSync('git', ['rev-parse', '--verify', '$ref^{commit}']);
  if (check.exitCode != 0) {
    stderr.writeln('Error: "$ref" is not a valid git ref.');
    stderr.writeln('Check: git tag -l  or  git log --oneline -5');
    exit(1);
  }

  final result = Process.runSync('git', [
    'log', '$ref..HEAD', '--oneline', '--no-decorate',
  ]);
  if (result.exitCode != 0) {
    stderr.writeln('git log failed: ${result.stderr}');
    exit(1);
  }

  final commits = (result.stdout as String).trim();
  if (commits.isEmpty) {
    stderr.writeln('No commits since $ref.');
    return;
  }

  final lines = commits.split('\n');
  final label = ref.startsWith('v') ? ref.substring(1) : ref;

  stderr.writeln('${lines.length} commits since $ref');
  stderr.writeln('');

  print('<details><summary>Commits since $label (${lines.length})</summary>');
  print('');
  for (final line in lines) {
    print('- $line');
  }
  print('');
  print('</details>');
}
