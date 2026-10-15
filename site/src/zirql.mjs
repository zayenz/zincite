import zirql from '../../assets/syntax/zirql.tmLanguage.json' with { type: 'json' };

// Shared Zirql highlighting, with Zincite's MiniZinc-oriented surface syntax.
// These additions describe spelling, not which operations a query may execute.
export default {
  ...zirql,
  aliases: ['zq'],
  patterns: [
    { name: 'comment.line.zirql', match: '%.*$' },
    { name: 'comment.block.zirql', begin: String.raw`/\*`, end: String.raw`\*/` },
    { name: 'string.quoted.single.zirql', begin: "'", end: "'", patterns: [{ name: 'constant.character.escape.zirql', match: String.raw`\\.` }] },
    { name: 'constant.numeric.zirql', match: String.raw`\b(?:0[xX][0-9a-fA-F]+|0[oO][0-7]+)\b` },
    { name: 'keyword.operator.zirql', match: String.raw`<->|/\\|\\/` },
    ...zirql.patterns,
    { name: 'keyword.control.zirql', match: String.raw`(?<![\w.])\b(?:source|create|replace|emit|emit_document|then|elseif|endif)\b(?!\s*:)` },
    { name: 'keyword.operator.word.zirql', match: String.raw`(?<![\w.])\b(?:xor|union|intersect|diff|subset|superset|symdiff)\b(?!\s*:)` },
    { name: 'support.function.zirql', match: String.raw`(?<![\w.])\b(?:one|literal|query|json)\b(?!\s*:)` },
  ],
};
