import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import minizinc from './src/minizinc.mjs';

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
    description: 'Format, lint and query MiniZinc source files.',
    logo: { light: '../assets/zincite-logo-dark.svg', dark: '../assets/zincite-logo-light.svg', replacesTitle: true, alt: 'Zincite' },
    favicon: '/favicon.svg',
    customCss: ['./src/styles/custom.css'],
    social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/zayenz/zincite' }],
    sidebar: [
      { label: 'Start here', items: ['guides/getting-started', 'guides/support'] },
      { label: 'Formatting', items: ['guides/formatting', 'guides/formatting-style', 'guides/format-on-save'] },
      { label: 'Linting', items: ['guides/linting'] },
      { label: 'Queries and data', items: ['guides/queries', 'guides/query-cookbook', 'guides/data-edits'] },
      { label: 'Reference', items: ['guides/cli', 'guides/reference'] },
      { label: 'All projects on zayenz.se', link: 'https://zayenz.se/projects/' },
    ],
    expressiveCode: { shiki: { langs: [minizinc], langAlias: { 'rust,ignore': 'rust' } } },
  })],
});
