# Live Smoke CLI And Provider Config Pre-Fix Audit

## Snapshot

- Git HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Git tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status: dirty before this fix. Existing modified and untracked files span CLI, kernel, thread runtime, conformance tests, docs, provider config, permission, and ecosystem work.
- Audit scope: issues observed while running live Agent-OS frontend and backend smoke tasks against OpenAI-compatible and Anthropic-compatible provider endpoints.

## Validation Already Run

- `cargo run -p agent-os-cli --quiet -- --help`: passed before the live smoke tasks.
- Live OpenAI-compatible smoke initially failed at CLI argument parsing with `unknown chat option OS` when a multi-word task value was not preserved as one argv.
- Live provider config loading initially failed with `parse global provider config ... expected value at line 1 column 1` when the config file was written as UTF-8 with BOM.
- After manual command/config correction outside the repo, both live smoke tasks completed and produced replayable state databases.

## Current-Contract Findings

1. Global provider config parsing rejects UTF-8 BOM-prefixed JSON. On Windows, common PowerShell config authoring commands can create a BOM-prefixed `providers.json`, and the CLI currently surfaces this as a low-context JSON parse failure.
2. `agent-os chat --task` accepts only one argv as the task value. A user who passes a simple unquoted multi-word task gets an opaque `unknown chat option <word>` error even when the rest of the command is otherwise valid.

## Future-Roadmap Gaps

- A first-class provider configuration management command is still out of scope for this fix.
- The CLI cannot and should not infer arbitrary shell quoting intent inside task text that itself starts with option-like tokens. Quoted task strings remain the canonical exact form.

## Intended Fix Scope

- Add focused CLI unit coverage for BOM-prefixed provider config JSON.
- Add focused CLI unit coverage for multi-token chat task values.
- Make provider config parsing accept a leading UTF-8 BOM while still rejecting malformed JSON.
- Make `chat --task` and positional chat task parsing collect adjacent non-option tokens into one task string.
- Update help text only if the public CLI contract needs a small wording clarification.
