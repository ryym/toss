# Git Conventions

- Commit only if Git pre-commit check passes which runs linters, formatters, etc (see `lefthook.yml` for details).
  - Never use `-n` or `--no-verify` on commit. If your change breaks pre-commit check, fix code or tests and commit them together.
- Commit changes in focused chunks per intention. Avoid large commits that mix multiple concerns.

## Commit message format

Write the message in present tense sentence.

Use conventional commits format. Valid prefixes: feat, fix, refactor, docs, chore, perf.
Treat test code as the same as production code. For example, use `feat` when adding test cases.
