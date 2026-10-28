import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import minizinc from './src/minizinc.mjs';
import zirql from './src/zirql.mjs';

const base = process.env.SITE_BASE || '/';
const site = process.env.SITE_URL || 'https://zincite.example';
if (!/^\/(?:[a-zA-Z0-9_-]+\/)*[a-zA-Z0-9_-]*$/.test(base)) {
  throw new Error('SITE_BASE must be / or a path such as /zincite');
}
const url = new URL(site);
if (!['https:', 'http:'].includes(url.protocol) || url.pathname !== '/' || url.search || url.hash) {
  throw new Error('SITE_URL must be an HTTP(S) origin; set its path with SITE_BASE');
}

export default defineConfig({
  site, base, trailingSlash: 'always',
  integrations: [starlight({
    title: 'Zincite',
    description: 'Format, lint and query MiniZinc models, and check FlatZinc candidates.',
    logo: { light: '../assets/zincite-logo-dark.svg', dark: '../assets/zincite-logo-light.svg', replacesTitle: true, alt: 'Zincite' },
    favicon: '/favicon.svg',
    customCss: ['./src/styles/custom.css'],
    social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/zayenz/zincite' }],
    sidebar: [
      { label: 'Start here', items: [{ label: 'Overview', slug: '' }, 'guides/getting-started', 'guides/first-model', 'guides/support'] },
      { label: 'Formatting', items: ['guides/formatting', 'guides/formatting-style', 'guides/format-on-save'] },
      { label: 'Linting', items: [
        { label: 'Running the linter', slug: 'guides/linting' },
        { label: 'Conventions', slug: 'guides/lint-conventions' },
        { label: 'Scope and unused names', slug: 'guides/lint-scope' },
        { label: 'Arrays', slug: 'guides/lint-arrays' },
        { label: 'Domains and preconditions', slug: 'guides/lint-domains' },
        { label: 'Model formulation', slug: 'guides/lint-formulation' },
        { label: 'Comprehensions and flattening', slug: 'guides/lint-flattening' },
        { label: 'Search and symmetry breaking', slug: 'guides/lint-search' },
      ] },
      { label: 'Queries and data', items: ['guides/queries', 'guides/query-cookbook', 'guides/data-edits'] },
      { label: 'Candidate checking', items: ['guides/checker', 'guides/checker-architecture'] },
      { label: 'Reference', items: [
        'guides/cli',
        { label: 'Query expressions', slug: 'guides/query-comprehension-contract' },
        'guides/reference',
      ] },
      { label: 'Zirql library', items: [
        { label: 'Overview and alphabetical index', link: 'https://zayenz.se/zirql/guides/library-reference/' },
        { label: 'Scalars and strings', link: 'https://zayenz.se/zirql/guides/library-strings/' },
        { label: 'Presence and lookup', link: 'https://zayenz.se/zirql/guides/library-presence/' },
        { label: 'Operators and access', link: 'https://zayenz.se/zirql/guides/library-operators/' },
        { label: 'Collections and reductions', link: 'https://zayenz.se/zirql/guides/library-collections/' },
        { label: 'Collection pipeline stages', link: 'https://zayenz.se/zirql/guides/library-pipelines/' },
        { label: 'Ordering and extrema', link: 'https://zayenz.se/zirql/guides/library-ordering/' },
        { label: 'Grouping and neighbourhoods', link: 'https://zayenz.se/zirql/guides/library-grouping/' },
        { label: 'Functions and control flow', link: 'https://zayenz.se/zirql/guides/library-control/' },
        { label: 'Shared native composition', link: 'https://zayenz.se/zirql/guides/library-native/' },
        { label: 'JSON and Markdown rendering', link: 'https://zayenz.se/zirql/guides/library-rendering/' },
        { label: 'Language and scope', link: 'https://zayenz.se/zirql/guides/language-reference/' },
      ] },
      { label: 'All projects on zayenz.se', link: 'https://zayenz.se/projects/' },
    ],
    expressiveCode: { shiki: { langs: [minizinc, zirql], langAlias: { 'rust,ignore': 'rust' } } },
  })],
});
