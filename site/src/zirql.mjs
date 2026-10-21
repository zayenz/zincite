import zirql from '../../assets/syntax/zirql.tmLanguage.json' with { type: 'json' };

// Shared query highlighting with Zincite's statement and edit keywords.
// These additions describe spelling, not which operations a query may execute.
export default {
  ...zirql,
  aliases: ['zq'],
  patterns: [
    // Escaped quoted names and 0O are deliberate legacy highlighting allowances.
    // They do not imply parser support.
    { name: 'string.quoted.single.zirql', begin: "'", end: "'", patterns: [{ name: 'constant.character.escape.zirql', match: String.raw`\\.` }] },
    { name: 'constant.numeric.zirql', match: String.raw`\b0O[0-7]+\b` },
    { name: 'keyword.operator.zirql', match: String.raw`<->` },
    ...zirql.patterns,
    { name: 'keyword.control.zirql', match: String.raw`(?<![\w.])\b(?:create|replace|append|scale_enum|emit|emit_document)\b(?!\s*:)` },
    { name: 'keyword.operator.word.zirql', match: String.raw`(?<![\w.])\b(?:xor|union|intersect|diff|subset|superset|symdiff)\b(?!\s*:)` },
    { name: 'support.function.zirql', match: String.raw`(?<![\w.])\b(?:one|query|json|clone_members|clone_elements|repeat_elements)\b(?!\s*:)` },
  ],
};
