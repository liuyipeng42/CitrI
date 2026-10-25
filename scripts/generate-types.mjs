import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const result = spawnSync('cargo', ['test', 'export_bindings', '--quiet'], {
  cwd: fileURLToPath(new URL('../src-tauri/', import.meta.url)),
  env: { ...process.env, TS_RS_EXPORT_DIR: '../src/generated' },
  stdio: 'inherit',
})

if (result.error) {
  console.error(result.error.message)
  process.exit(1)
}
process.exit(result.status ?? 1)
