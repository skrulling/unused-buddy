# unused-buddy

Fast Rust CLI for finding, listing, and safely removing unused JS/TS code.

## What it does (v1)

- Detects unused exports (`UnusedExport`)
- Detects unreachable files (`UnreachableFile`)
- Flags uncertain dynamic patterns (`Uncertain`)
- Supports JS/TS: `.js`, `.ts`, `.jsx`, `.tsx`
- Supports Node-style ESM + CJS imports
- Supports TypeScript `baseUrl` and `paths` from `tsconfig.json`

## Install and run

### Build

```bash
cargo build --release
```

### Run

```bash
cargo run -- scan .
cargo run -- list .
cargo run -- remove .
```

## Commands

- `unused-buddy scan [path]`
- `unused-buddy list [path]`
- `unused-buddy remove [path]`
- `unused-buddy help [command]`

### Global flags

- `--config <path>`
- `--format human|ai` (default: `human`)
- `--color auto|always|never` (default: `auto`)
- `--entry <file>` (repeatable)
- `--include <glob>` (repeatable)
- `--exclude <glob>` (repeatable)
- `--max-workers <n>`
- `--fail-on-findings`

### Remove-specific flags

- `--fix` apply file removals
- `--yes` skip confirmation gating

## Help modes

### Human help

```bash
unused-buddy --help
unused-buddy scan --help
unused-buddy remove --help
```

### AI help (machine-readable)

```bash
unused-buddy --help --format ai
unused-buddy scan --help --format ai
unused-buddy remove --help --format ai
```

AI help output is compact deterministic JSON with top-level keys:

- `n`: command name
- `d`: description
- `u`: usage
- `s`: subcommands
- `f`: flags
- `e`: examples
- `x`: exit codes

## Output formats

### Human mode (`--format human`)

Readable, color-coded output with fallback tags:

- `[UF]` unreachable file
- `[UE]` unused export
- `[UC]` uncertain finding

### AI mode (`--format ai`)

Compact NDJSON, one finding per line, keys:

- `i` id
- `k` kind (`uf`, `ue`, `uc`)
- `f` file
- `s` symbol (optional)
- `r` reason
- `l` line (optional)
- `c` col (optional)
- `x` fixable (`0|1`)
- `q` confidence

## Color behavior and portability

Color is terminal-capability aware and shell-agnostic.

Order of behavior:

1. `--color never` disables color
2. `--color always` forces color
3. `--color auto` uses environment + TTY detection

In `auto`, these conventions are respected:

- `NO_COLOR` disables color
- `CLICOLOR=0` disables color
- `CLICOLOR_FORCE=1` forces color
- `FORCE_COLOR=1` forces color
- `TERM=dumb` disables color
- non-TTY stdout disables color unless forced

## Safe remove workflow

Default remove is report-only.

```bash
unused-buddy remove .
```

Apply removals:

```bash
unused-buddy remove . --fix --yes
```

Safety guarantees in v1:

- Only safe `UnreachableFile` findings are auto-removed
- Uncertain/risky findings are never auto-removed
- Unused exports are report-only (no symbol rewriting)

## Configuration

Create `unused-buddy.toml` in project root.

```toml
include = ["src/**/*.{js,ts,jsx,tsx}"]
exclude = [
  "node_modules/**",
  "dist/**",
  "build/**",
  "coverage/**",
  ".next/**",
  "out/**",
  "**/*.d.ts",
  "**/*.test.*",
  "**/*.spec.*",
  "**/__tests__/**"
]
entry = ["src/index.ts"]
extensions = ["js", "ts", "jsx", "tsx"]
max_workers = 0
format = "human"
color = "auto"
fix_mode = "files_only"
```

Precedence: `CLI flags > config file > defaults`.

## Defaults

- Default format: `human`
- Default color: `auto`
- Default include: `src/**/*.{js,ts,jsx,tsx}`
- Default excludes: build artifacts, test/spec files, declarations, vendor dirs

## Exit behavior

- Exit `0` on successful execution
- Exit non-zero on runtime/config errors
- With `--fail-on-findings`, exits non-zero when findings exist

## Current limitations

- Single-package scope in v1 (no cross-package monorepo graph)
- Conservative static analysis for dynamic runtime patterns
- No symbol-level auto-fix for unused exports yet

## Benchmarking

Run the benchmark suite:

```bash
cargo bench --bench scan_bench
```

This benchmark generates a deterministic synthetic JS/TS project (roughly 100k LOC) and measures full `scan` throughput.

Tips:

- Run on a warm machine (close background-heavy apps) for stable numbers.
- Repeat runs and compare medians, not single best samples.
- For quick validation after changes:

```bash
cargo bench --bench scan_bench -- --sample-size 10
```
