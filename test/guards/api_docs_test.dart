// Every element of the public API has a documentation comment, as pub.dev's
// documentation check counts it: an exported top-level name, and every public
// member of an exported type, the constructor Dart adds to a class that
// declares none included. The analyzer's `public_member_api_docs` lint reads
// only what the source declares, so it misses that constructor; this wall
// does not. A member that overrides a documented one inherits its comment.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:test/test.dart';

import 'api_model.dart';

bool _documented(Element element) =>
    (element.documentationComment ?? '').trim().isNotEmpty;

/// Whether [member] of [owner] overrides a member some supertype documents.
bool _inheritsDocs(InterfaceElement owner, Element member) {
  final name = member.name;
  if (name == null || member is ConstructorElement) return false;
  for (final type in owner.allSupertypes) {
    final element = type.element;
    for (final candidate in <Element>[
      ...element.methods,
      ...element.getters,
      ...element.setters,
      ...element.fields,
    ]) {
      if (candidate.name == name && _documented(candidate)) return true;
    }
  }
  return false;
}

/// Whether an app can call [constructor]: an enum's never, nor a class that
/// no other library may make or extend (sealed, or abstract and interface or
/// final).
bool _callable(ConstructorElement constructor) {
  final owner = constructor.enclosingElement;
  if (owner is EnumElement) return false;
  if (owner is ClassElement) {
    if (owner.isSealed) return false;
    if (owner.isAbstract && (owner.isInterface || owner.isFinal)) return false;
  }
  return true;
}

/// How pub.dev names [member] when it has no documentation comment, or null
/// when it has one, inherits one, or is a constructor no app can call.
String? _missing(ApiMember member) {
  final element = member.element;
  if (_documented(element) || _inheritsDocs(member.owner, element)) return null;
  if (element is! ConstructorElement) return member.label;
  if (!_callable(element)) return null;
  return '${member.owner.displayName}.${element.name ?? 'new'}';
}

/// The public API elements of [model] with no documentation comment.
List<String> undocumented(ApiModel model) => [
  for (final MapEntry(:key, :value) in model.all.entries)
    if (!_documented(value)) key,
  ...model.members.map(_missing).nonNulls,
];

void main() {
  test('every public API element has a documentation comment', () async {
    expect(undocumented(await ApiModel.load()), isEmpty);
  });
}
