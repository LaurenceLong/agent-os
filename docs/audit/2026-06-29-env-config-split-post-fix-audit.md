# Env Config Split Post-Fix Audit

## Validation Results

- `cargo fmt --all`: passed
- `cargo test -p agent-os-cli -p agent-os-thread -p agent-os-kernel -p agent-os-sys`: passed
- `cargo test --workspace`: passed
- `cargo clippy --workspace --all-targets -- -D warnings`: passed
- Current-contract search for old runtime `LLM_*` keys and private endpoint text
  under `crates`, `README.md`, `docs/10-kernel-design`,
  `docs/20-implementation`, `.env.example`, and `distros/providers.example.json`:
  no old runtime env keys or private endpoint examples remain.

## Changed Files

- `crates/agent-os-cli/src/provider_config.rs`
- `crates/agent-os-cli/src/chat.rs`
- `crates/agent-os-cli/src/args.rs`
- `crates/agent-os-cli/src/chat/tests.rs`
- `crates/agent-os-cli/src/main.rs`
- `crates/agent-os-cli/Cargo.toml`
- `Cargo.lock`
- `crates/agent-os-sys/src/provider.rs`
- `crates/agent-os-kernel/src/profile_seed/provider.rs`
- `crates/agent-os-thread/src/openai/client.rs`
- `crates/agent-os-thread/src/openai/tests/live.rs`
- `crates/agent-os-thread/src/openai/tests/unit.rs`
- `README.md`
- `docs/10-kernel-design/provider-system.md`
- `docs/20-implementation/production-roadmap.md`
- `.env.example`
- `distros/providers.example.json`

## Implemented Fixes

- Runtime `agent-os chat` now resolves provider settings from a user-level global
  `providers.json` file instead of repository-local `LLM_*` environment
  variables.
- The chat CLI now selects providers by `--provider`; flat provider override
  flags were removed from the current contract.
- Provider config supports multiple named providers with required `api_key`,
  `base_url`, `model`, and `api_style` fields.
- Kernel provider seed credentials now reference `local_config` instead of an
  environment variable.
- Live LLM tests use explicit `AGENT_OS_LIVE_OPENAI_*` and
  `AGENT_OS_LIVE_ANTHROPIC_*` test-only variables, with no runtime `LLM_*`
  reuse and no default provider endpoint or model fallback.
- `.env.example` now documents only live test variables.
- `distros/providers.example.json` documents the global runtime provider config
  shape without private endpoints or real secrets.
- README and roadmap examples now use public placeholder endpoints and the new
  test/runtime split.

## Forward-Only Notes

- No old `LLM_*`, `OPENAI_*`, or `AGENT_OS_MODEL` runtime compatibility path was
  retained.
- Runtime provider settings are intentionally outside the repository so checkout
  replacement and Agent-OS upgrades do not remove user provider configuration.
- `api_style` is required in global provider config to avoid implicit adapter
  selection.

## Remaining Gaps

- The global provider config is currently read by the CLI runtime path. A future
  provider-system crate can move this loader behind the system-level provider
  control plane without changing the user-level config location.
