import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const site = fileURLToPath(new URL('../', import.meta.url));
const root = resolve(site, '..');
const guides = [
  'getting-started', 'formatting', 'formatting-style', 'format-on-save',
  'linting', 'queries', 'query-cookbook', 'data-edits', 'cli', 'support', 'reference',
];
const output = resolve(site, 'src/content/docs/guides');
await mkdir(output, { recursive: true });

for (const name of guides) {
  const source = resolve(root, `docs/${name}.md`);
  const markdown = await readFile(source, 'utf8');
  const title = markdown.match(/^# (.+)\r?\n/);
  if (!title) throw new Error(`Missing page title in ${source}`);
  const body = markdown.slice(title[0].length).replace(/\]\(([^\s)]+)\)/g, (link, target) => {
    if (/^(?:[a-z]+:|#|\/)/i.test(target)) return link;
    const [path, anchor = ''] = target.split('#');
    const resolved = relative(root, resolve(root, 'docs', path));
    const slug = resolved.match(/^docs\/([\w-]+)\.md$/)?.[1];
    if (slug && guides.includes(slug)) return `](../${slug}/${anchor ? `#${anchor}` : ''})`;
    if (resolved === 'README.md') return `](../../${anchor ? `#${anchor}` : ''})`;
    return `](https://github.com/zayenz/zincite/blob/main/${resolved}${anchor ? `#${anchor}` : ''})`;
  });
  await writeFile(resolve(output, `${name}.md`), `---\ntitle: ${JSON.stringify(title[1])}\neditUrl: https://github.com/zayenz/zincite/edit/main/docs/${name}.md\n---\n${body}`);
}

await mkdir(resolve(site, 'public'), { recursive: true });
for (const asset of ['favicon.svg', 'apple-touch-icon.png']) {
  await copyFile(resolve(root, 'assets', asset), resolve(site, 'public', asset));
}

const gettingStarted = await readFile(resolve(root, 'docs/getting-started.md'), 'utf8');
const install = gettingStarted.match(/```sh\n[\s\S]*?```/)?.[0];
const examples = gettingStarted.match(/## Try the commands[\s\S]*?(```sh\n[\s\S]*?```)/)?.[1];
if (!install || !examples) throw new Error('Missing installation or examples in docs/getting-started.md');
const landing = await readFile(resolve(site, 'src/landing.mdx'), 'utf8');
await writeFile(resolve(site, 'src/content/docs/index.mdx'), landing
  .replace('<!-- install-commands -->', install)
  .replace('<!-- example-commands -->', examples));
