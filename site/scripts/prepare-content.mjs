import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderLintExamples } from './lint-examples.mjs';

const site = fileURLToPath(new URL('../', import.meta.url));
const root = resolve(site, '..');
const guides = [
  'getting-started', 'first-model', 'formatting', 'formatting-style', 'format-on-save',
  'linting', 'lint-conventions', 'lint-scope', 'lint-arrays', 'lint-domains',
  'lint-formulation', 'lint-flattening', 'lint-search',
  'queries', 'query-cookbook', 'query-comprehension-contract', 'data-edits', 'cli', 'support', 'reference',
];
const output = resolve(site, 'src/content/docs/guides');
await mkdir(output, { recursive: true });

for (const name of guides) {
  const source = resolve(root, `docs/${name}.md`);
  const markdown = renderLintExamples(await readFile(source, 'utf8'));
  const title = markdown.match(/^# (.+)\r?\n/);
  if (!title) throw new Error(`Missing page title in ${source}`);
  const body = markdown.slice(title[0].length).replace(/\]\(([^\s)]+)\)/g, (link, target) => {
    if (/^(?:[a-z]+:|#|\/)/i.test(target)) return link;
    const [path, anchor = ''] = target.split('#');
    const resolved = relative(root, resolve(root, 'docs', path));
    const slug = resolved.match(/^docs\/([\w-]+)\.md$/)?.[1];
    if (slug && guides.includes(slug)) return `](../${slug}/${anchor ? `#${anchor}` : ''})`;
    if (resolved === 'README.md') return `](../../${anchor ? `#${anchor}` : ''})`;
    return `](https://github.com/zayenz/zincite/blob/v0.1.2/${resolved}${anchor ? `#${anchor}` : ''})`;
  });
  await writeFile(resolve(output, `${name}.md`), `---\ntitle: ${JSON.stringify(title[1])}\neditUrl: false\n---\n${body}`);
}

await mkdir(resolve(site, 'public'), { recursive: true });
for (const asset of ['favicon.svg', 'apple-touch-icon.png']) {
  await copyFile(resolve(root, 'assets', asset), resolve(site, 'public', asset));
}

const gettingStarted = await readFile(resolve(root, 'docs/getting-started.md'), 'utf8');
const install = gettingStarted.match(/```sh\n[\s\S]*?```/)?.[0];
if (!install) throw new Error('Missing installation in docs/getting-started.md');
const landing = await readFile(resolve(site, 'src/landing.mdx'), 'utf8');
await writeFile(resolve(site, 'src/content/docs/index.mdx'), landing
  .replace('<!-- install-commands -->', install));
