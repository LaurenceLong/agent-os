# Env Config Split Pre-Fix Audit

## Git State

- HEAD: `9a8aec5769abdac916a35a618677e0b5fc781486`
- Tree: `c6cd015b14c9d13313f32826c4537bb2dceced4f`
- Worktree status before this fix: `.env.example` is untracked from the prior template creation.

## Scope

Audit the environment variable contract for runtime Agent-OS provider configuration
and live LLM test configuration. The immediate goal is to prevent build/test
configuration from sharing the same `LLM_*` runtime keys and to remove private
endpoint examples from public templates and docs.

## Current-Contract Findings

- `agent-os-cli chat` uses `LLM_API_KEY`, `LLM_BASE_URL`, `LLM_MODEL`, and
  `LLM_API_STYLE` as its runtime configuration surface.
- Live LLM tests in `agent-os-thread` currently reuse `LLM_API_KEY` and
  `LLM_MODEL`, while also accepting provider-specific base URL overrides. This
  makes test configuration look like runtime provider configuration.
- The root `.env.example` currently documents only runtime `LLM_*` fields, which
  does not make the test/runtime split explicit.
- `README.md` and `docs/20-implementation/production-roadmap.md` include private
  endpoint examples and show live tests using runtime `LLM_*` variables.

## Future-Roadmap Gaps

- Runtime provider configuration should eventually align with the multi-provider
  profile model rather than a single flat env surface. This fix only separates
  current runtime and live-test environment namespaces.

## Validation Already Run

- `rg` searches for the relevant env variable names across `crates`, `README.md`,
  `docs`, and `.env.example`.
- `git status --short`.

## Intended Fix Scope

- Rename live LLM test env variables to an explicit test-only namespace.
- Update `.env.example` so runtime and live-test settings are separate.
- Replace private endpoint examples in public docs with non-private placeholders.
- Keep compatibility shims out of the test code; use one canonical live-test env
  namespace.
