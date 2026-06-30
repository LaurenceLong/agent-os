# Run Command Env Post-Fix Audit

Date: 2026-06-30

## Implemented Fixes

1. Added an optional `env` object to the `run_command` kernel driver input.
2. Applied `env` key/value pairs directly to the spawned command process.
3. Updated model-visible tool schemas and profile seed schemas to expose
   `env`.
4. Updated the system prompt so models can use `run_command(program, args,
   env?)` for focused environment overrides such as `PYTHONPATH`.
5. Extended conformance coverage so `run_command.env` remains part of the
   public model-visible tool contract.

## Changed Files

- `crates/agent-os-kernel/src/tools/driver/workspace.rs`
- `crates/agent-os-kernel/src/profile_seed/tools/filesystem.rs`
- `crates/agent-os-kernel/src/profile_seed/tool_schemas.rs`
- `crates/agent-os-thread/src/openai/prompt.rs`
- `crates/agent-os-conformance/tests/integration/tool_broker.rs`

## Validation Results

```text
cargo test -p agent-os-conformance tool_broker_integration_runs_all_model_visible_tool_families -- --nocapture
result: 1 passed

cargo clippy --workspace --all-targets -- -D warnings
result: passed
```

## Forward-Only Notes

`env` is the canonical command-level environment override. It keeps environment
control with the workspace command tool and avoids adding benchmark-specific
wrapper behavior elsewhere in the runtime.

## Remaining Gaps

- No remaining current-contract gap for command-level environment overrides.
