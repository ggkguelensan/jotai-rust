import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
for (const [directory, slug] of [
  ['yew-counter', 'counter'],
  ['yew-spreadsheet', 'spreadsheet'],
]) {
  const result = spawnSync('trunk', [
    'build', '--dist', resolve(root, 'docs/public/examples', slug), '--public-url', './',
  ], { cwd: resolve(root, 'examples', directory), stdio: 'inherit' });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  if (result.status !== 0) process.exit(result.status ?? 1);
}
