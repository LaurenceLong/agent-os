# Live Smoke CLI And Provider Config Post-Fix Audit

## Snapshot

- Git HEAD at fix start: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree at fix start: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status: dirty before and after this fix. This change intentionally stayed scoped to CLI argument parsing, provider config parsing, focused tests, and audit records.

## Implemented Fixes

1. Added a shared CLI task-argument collection path for `run --task`, `code --task`, `chat --task`, and positional `chat` tasks. The parser now joins adjacent non-option tokens until the next option-like argument.
2. Added focused parser tests for multi-token task values in `run`, `code`, `chat --task`, and positional `chat`.
3. Added a provider config parse boundary that accepts a leading UTF-8 BOM before deserializing JSON.
4. Added focused provider config coverage for BOM-prefixed JSON while preserving validation of the current provider schema.

## Validation Results

- `cargo test -p agent-os-cli --bin agent-os task_flag_collects -- --nocapture`: failed before the fix with `unknown ... option` errors, then passed after the fix with 3/3 tests passing.
- `cargo test -p agent-os-cli --bin agent-os global_provider_config_loads_utf8_bom_json -- --nocapture`: failed before the fix with `expected value at line 1 column 1`, then passed after the fix.
- `cargo fmt --check`: passed.
- `cargo test -p agent-os-cli --bin agent-os`: passed, 18/18 tests.
- `cargo clippy -p agent-os-cli --all-targets -- -D warnings`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- BOM provider config smoke: `agent-os chat --provider default` loaded a BOM-prefixed temp `providers.json`, printed provider/model, and exited without parse error.
- Multi-token task smoke: `agent-os run --task Write task report --output result.md` completed and wrote a task report containing `Write task report`.

## Forward-Only Notes

- This keeps global provider config as the canonical runtime provider source. No `LLM_*`, `--api-key`, `--api-base`, or `--model` compatibility path was reintroduced.
- The parser accepts common shell-tokenized task text but still treats option-like tokens as CLI boundaries. Exact task text containing leading `-` tokens should be quoted by the caller.
- UTF-8 BOM handling is a config file decoding concern, not a legacy provider schema fallback. The JSON shape remains the current contract.

## Remaining Gaps

- A first-class `agent-os provider` config authoring command remains outside this fix.
- Live provider spend tests were not rerun because this fix targeted local CLI/config parsing and was covered by non-network smoke plus the earlier live run evidence.
