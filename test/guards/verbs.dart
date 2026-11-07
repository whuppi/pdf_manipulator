// The verbs a task or a method may start with. A new verb is a new row, so a
// name that drifts to a second word for the same idea fails until someone
// decides.

/// Verbs that start the name of a task or an action.
const verbs = <String>{
  'abort',
  'add',
  'append',
  'apply',
  'build',
  'cancel',
  'classify',
  'clear',
  'close',
  'compress',
  'convert',
  'crop',
  'dispose',
  'ensure',
  'export',
  'extract',
  'flatten',
  'keep',
  'mark',
  'merge',
  'move',
  'open',
  'read',
  'redact',
  'reduce',
  'register',
  'remove',
  'render',
  'reorder',
  'resave',
  'rewrite',
  'rotate',
  'run',
  'sanitize',
  'save',
  'search',
  'set',
  'sign',
  'split',
  'take',
  'unembed',
  'validate',
  'verify',
  'write',
};

/// The first word of a camelCase [name], lowercase.
String firstWord(String name) =>
    RegExp(r'^[a-z0-9]+').firstMatch(name)?.group(0) ?? name;

/// Whether [name] starts with a verb of [verbs].
bool startsWithVerb(String name) => verbs.contains(firstWord(name));
