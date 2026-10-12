// Presentation grammar only. Language acceptance belongs to zincite-syntax.
export default {
  name: 'minizinc', scopeName: 'source.minizinc', aliases: ['mzn', 'dzn'],
  patterns: [
    { name: 'comment.line.percentage.minizinc', match: '%.*$' },
    { name: 'comment.block.minizinc', begin: '/\\*', end: '\\*/' },
    { name: 'string.quoted.double.minizinc', begin: '"', end: '"', patterns: [{ name: 'constant.character.escape.minizinc', match: '\\\\.' }] },
    { name: 'string.quoted.single.minizinc', begin: "'", end: "'" },
    { name: 'keyword.control.minizinc', match: '\\b(?:include|constraint|solve|satisfy|minimize|maximize|output|if|then|else|elseif|endif|let|in|where|function|predicate|test|annotation|enum|type|of|array|set|list|tuple|record|var|par|opt|any)\\b' },
    { name: 'storage.type.minizinc', match: '\\b(?:int|float|bool|string)\\b' },
    { name: 'constant.language.minizinc', match: '\\b(?:true|false)\\b' },
    { name: 'constant.numeric.minizinc', match: '\\b(?:0[xX][0-9a-fA-F]+|0[oO][0-7]+|[0-9]+(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)\\b' },
    { name: 'entity.name.function.minizinc', match: '\\b[a-zA-Z_][a-zA-Z_0-9]*(?=\\s*\\()' },
    { name: 'keyword.operator.minizinc', match: '(?:::|\\.\\.|[=<>+*/\\-]|\\b(?:div|mod|not|xor|union|intersect|diff|subset|superset)\\b)' },
  ],
};
