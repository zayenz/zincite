import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const manifest = fileURLToPath(new URL('../../Cargo.toml', import.meta.url));
let executable;
let stdlib;

function buildLinter() {
  if (executable) return;
  const messages = execFileSync('cargo', [
    'build', '--quiet', '--manifest-path', manifest,
    '--package', 'zincite-lint', '--bin', 'zincite-lint', '--message-format=json',
  ], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
  executable = messages.trim().split('\n').map(line => JSON.parse(line))
    .find(message => message.reason === 'compiler-artifact'
      && message.target.name === 'zincite-lint' && message.executable)?.executable;
  if (!executable) throw new Error('Cargo did not produce the zincite-lint executable');

  stdlib = process.env.MZN_STDLIB_DIR;
  if (!stdlib) {
    try {
      stdlib = JSON.parse(execFileSync('minizinc', ['--config-dirs'], {
        encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'],
      })).mznStdlibDir;
    } catch {
      throw new Error('Lint examples need MiniZinc on PATH or MZN_STDLIB_DIR pointing to its library');
    }
  }
  if (!stdlib) throw new Error('MiniZinc did not report its standard-library directory');
  stdlib = resolve(stdlib);
}

// A marker applies to the MiniZinc block immediately above it. Use the same
// source for the displayed example and the CLI input; keep only site output generated.
export function renderLintExamples(markdown) {
  const rendered = markdown.replace(
    /(```minizinc\n((?:(?!^```)[\s\S])*?)^```)\n\n<!-- zincite-lint: ([a-z][a-z-]*) -->/gm,
    (_, block, source, rule) => {
      buildLinter();
      const directory = mkdtempSync(join(tmpdir(), 'zincite-doc-lint-'));
      const filename = 'example.mzn';
      try {
        writeFileSync(join(directory, filename), source);
        const result = spawnSync(executable, [
          '--isolated', '--rules', rule, '--stdlib-dir', stdlib, filename,
        ], { cwd: directory, encoding: 'utf8' });
        if (result.error) throw result.error;
        if (result.status !== 1 || !result.stderr.includes(`warning [${rule}]:`)) {
          throw new Error(`Lint example '${rule}' did not produce its warning (exit ${result.status}):\n${result.stderr}`);
        }
        return `${block}\n\nSave the example as \`${filename}\`. With the
[standard library configured](linting.md#model-context-and-source-preservation), run:

\`\`\`sh wrap
zincite lint --isolated --rules ${rule} ${filename}
\`\`\`

Zincite reports:

\`\`\`text wrap
${result.stderr.trimEnd()}
\`\`\``;
      } finally {
        rmSync(directory, { recursive: true, force: true });
      }
    },
  );
  if (rendered.includes('<!-- zincite-lint:')) {
    throw new Error('A zincite-lint marker must follow a MiniZinc code block, separated by a blank line');
  }
  return rendered;
}
