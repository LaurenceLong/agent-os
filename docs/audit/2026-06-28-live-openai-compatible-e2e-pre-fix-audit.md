# Live OpenAI-Compatible E2E Pre-Fix Audit

Date: 2026-06-28

## Code Identity

- Git HEAD: `716a8c13027495a20f942060db87e05469d714d4`
- Git tree at audit start: `90f24fef238cc10e8e88d47534cecec56034ea53`
- Worktree status at audit start: dirty with the forward-only roadmap
  implementation, local `.env`, and audit records.

## Scope

This audit covers OpenAI-compatible live LLM e2e tests in
`crates/agent-os-thread/src/openai/tests/live.rs` after configuring the local
BigModel Coding endpoint.

## Current-Contract Gaps

- The full-tool-surface and agent-control rejection tests mutate a cloned
  `AgentControlBlock` task local goal, but the runtime reloads the thread from
  kernel state. The live model therefore sees short placeholder task text
  instead of the detailed tool sequence.
- The control-plane test asks for a shared blackboard entry and the model chose
  `scope: global`, which exceeds the supervisor communication profile.
- The full-tool-surface test uses a manually constructed `Kernel::new()` while
  prompting the model to record inline evidence. Inline evidence requires an
  evidence blob store.
- The rejection test expects append-only-store rejection, but the user-visible
  task is too vague and the model explores unrelated tools until the runtime
  reaches max steps.

## Future-Roadmap Gap

This pass does not make live LLM tests deterministic. It aligns the live test
scenarios with the current kernel contract and keeps them as genuine live model
tests.

## Intended Fix Scope

- Store detailed live task goals in kernel state before the runtime starts.
- Use policy-allowed task-scoped blackboard entries in control-plane prompts.
- Construct the full-tool-surface kernel with local blob stores.
- Make rejection prompts narrowly request one append-only-store rejection action
  and assert the failed tool invocation directly.
- Preserve live OpenAI-compatible coverage for the configured provider.

## Validation Planned

- `cargo test -p agent-os-thread live_openai_compatible -- --ignored --nocapture --test-threads=1`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
