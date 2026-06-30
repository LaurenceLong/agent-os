# Chat Task File Post-Fix Audit

Date: 2026-06-30

## Implemented Fixes

1. Added `agent-os chat --task-file <path>` as the canonical file-backed batch
   task input for long benchmark prompts.
2. Rejected simultaneous `--task` and `--task-file` so one initial task source is
   authoritative.
3. Made `chat` sessions with an initial task run in batch mode and exit after
   completion instead of falling back into interactive stdin.
4. Updated CLI help JSON so the new contract is visible to scripts and users.

## Changed Files

- `crates/agent-os-cli/src/args.rs`
- `crates/agent-os-cli/src/chat.rs`
- `crates/agent-os-cli/src/chat/tests.rs`

## Validation Results

```text
cargo test -p agent-os-cli chat_ -- --nocapture
result: 6 passed

wsl.exe -d Ubuntu-22.04 --exec bash -lc "cd /mnt/d/work/ai_agents/coding-agent/agent-os && CARGO_TARGET_DIR=target/wsl2-linux cargo build -p agent-os-cli --bin agent-os && ./target/wsl2-linux/debug/agent-os --help >/tmp/agent-os-wsl-help.txt && head -20 /tmp/agent-os-wsl-help.txt"
result: Linux binary built and help JSON includes --task-file
```

## Forward-Only Notes

The file-backed task input is part of the current CLI batch contract. It is not
a fallback for interactive mode and does not preserve older ambiguous behavior
where an initial task could still leave the process waiting for stdin.

## Remaining Gaps

- No remaining current-contract gap for file-backed batch task input.
