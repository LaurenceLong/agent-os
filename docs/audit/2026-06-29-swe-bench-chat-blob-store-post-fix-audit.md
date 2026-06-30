# SWE-bench Chat Blob Store Post-Fix Audit

Date: 2026-06-29

## Implemented Fixes

1. Added CLI blob store initialization in `open_kernel()` so kernels opened by
   `agent-os-cli chat`, `run`, `code`, `resume`, and `status` can persist inline
   artifact and evidence bytes.
2. Added `agent-os-store` as a direct `agent-os-cli` dependency for
   `LocalBlobStore`.
3. Added a regression test proving a CLI-opened SQLite-backed kernel can attach
   inline evidence and commit an inline artifact.

## Changed Files

- `crates/agent-os-cli/Cargo.toml`
- `crates/agent-os-cli/src/support.rs`
- `Cargo.lock`
- `docs/audit/2026-06-29-swe-bench-chat-blob-store-pre-fix-audit.md`

## Validation Results

```text
cargo test -p agent-os-cli opened_kernel_persists_inline_evidence_and_artifact_blobs -- --nocapture
result: 1 passed

cargo test -p agent-os-cli
result: 19 passed

cargo fmt --all --check
result: passed

cargo clippy -p agent-os-cli --all-targets -- -D warnings
result: passed

cargo test --workspace
result: passed

cargo clippy --workspace --all-targets -- -D warnings
result: passed
```

## Benchmark Run Evidence

The private SWE-bench Lite 20-task run was executed with:

```text
model: tongyi/qwen3.6-plus
api style: anthropic-compatible
base URL: redacted private endpoint
```

The provider API key was written only to a temporary provider config during
execution and was deleted after the run. Local endpoint and deployment paths
belong in private operator memory, not the repository.

Detailed benchmark result rows and local artifact paths are intentionally kept
out of Git. They belong in local operator memory and ignored run directories.

## Forward-Only Notes

The blob-store fix is a current-contract requirement for any CLI path that can
attach inline command evidence or patch artifacts. It is not a compatibility
shim.

The benchmark result shows the next product gap clearly: Agent-OS needs an
official SWE-bench environment runner plus a stronger software-engineering
distribution prompt before this private suite can be used as a pass/fail
capability gate.

## Remaining Gaps

1. No official SWE-bench Docker score was produced.
2. `agent-os-cli chat` does not currently write provider audit logs for normal
   user sessions, which makes model-level failure analysis harder than the live
   e2e tests.
3. Several model failures were tool-schema discipline issues, such as non-array
   `run_command.args` and brittle `replace_text` calls. These should become
   focused distro/runtime tests before the next benchmark run.
