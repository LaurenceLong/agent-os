# Agent Optimization Statistics Study

Date: 2026-06-30

## Verdict

Agent-OS does not yet have one unified statistics system.

It has the right raw material:

- kernel events in the append-only SQLite event store
- provider stream sessions with accumulated `input_tokens`, `output_tokens`, and
  `cost`
- budget ledgers for token, tool-call, wall-time, cost, human-interrupt, and
  model-request budgets
- tool invocation events for proposed, started, completed, failed, and denied
  calls
- optional provider JSONL audit logs in tests and live e2e paths

The current implementation gap is not the desired design. Event JSON and
provider audit JSONL are raw evidence, not the statistics system. Agent-OS needs
a kernel-owned statistics projection that is maintained as part of normal state
transition handling, can be deterministically rebuilt from the event log, and
can be queried per turn, session, thread, task, agent, provider, model, tool,
and benchmark run.

The append-only event store remains the replay source of truth. The statistics
projection is the query surface.

## Current Agent-OS Signals

Current implementation already supports these direct or derived dimensions:

- Provider usage: input tokens, output tokens, cost, model requests.
- Budget usage: tokens, tool calls, wall time, cost, human interrupts, model
  requests.
- Tool outcomes: proposed, started, completed, failed, denied, risk level,
  evidence IDs, created/completed timestamps.
- Provider stream lifecycle: opened, usage updated, completed, failed,
  cancelled.
- Runtime loop health: max-step exhaustion, consecutive no-action model turns,
  final submission presence, blocked/noncompletion reason.
- Evidence and artifact quality: evidence count, artifact count, final
  submission evidence map, unverified claims, tests run/not run.
- Context pressure: context snapshot token estimates, context compaction token
  estimates.
- Benchmark result artifacts: exit code, patch existence, patch apply status,
  resolved status, test status buckets, command logs, patch path, state DB path.

Important limitation: provider cost is parsed as `0.0` in the current OpenAI and
Anthropic-compatible parsers, and cache/reasoning token fields are not modeled in
`ProviderUsage`.

## OpenCode Signals Worth Keeping

OpenCode's strongest reusable idea is that the assistant message is the durable
billing and tool accounting unit.

Useful dimensions:

- Per assistant message: cost, total/input/output/reasoning tokens, cache read,
  cache write.
- Per tool part: pending/running/completed/error status, input, output/error,
  metadata, start time, end time, compacted output marker.
- Per model: message count, input/output/cache-read/cache-write tokens, cost.
- Per session: total cost, total messages, total tokens, average cost per day,
  average tokens per session, median tokens per session.
- Per tool: call count by tool name.
- Pricing model: separate non-cached input, output, cache read, cache write, and
  reasoning-token cost; optionally switch pricing above a large-context
  threshold.
- Context pressure: overflow based on total tokens or input + output + cache
  read + cache write against usable context capacity.

Do not copy the whole message-storage shape into Agent-OS. The transferable
piece is the normalized usage record and the idea that every assistant step and
tool part is measurable without replaying natural language.

## Codex Signals Worth Keeping

Codex has the richer production telemetry spine.

Useful dimensions:

- Token usage: total, input, cached input, non-cached input, output, reasoning
  output, model context window, blended billable display total.
- Per-turn token deltas, not only session totals.
- API request metrics: count, duration, HTTP status, success flag, attempt,
  endpoint, auth/recovery metadata.
- Tool metrics: tool name, success flag, duration, argument length, output
  length, output line count, builtin vs MCP origin, MCP server metadata.
- Streaming metrics: SSE event count/duration/failure, WebSocket request and
  event count/duration.
- Model-serving latency: turn TTFT, turn TTFM, Responses API overhead,
  inference time, engine IAPI/service TTFT, engine IAPI/service TBT.
- Turn metrics: e2e duration, tool-call count per turn, network-proxy state,
  memory footprint.
- Product/runtime dimensions: goal lifecycle, plugin install suggestions,
  startup phases, thread skill counts and truncation.
- Persistence metrics: rollout item bytes, kept/dropped decision, turn bytes,
  append counts, measurement errors.

Codex's important design lesson is to separate durable audit facts from
low-cardinality metrics. Agent-OS should keep exact replay facts in kernel
events and build queryable metric projections from those facts.

## Recommended Agent-OS Metric Spine

Start with these canonical projection records:

1. `model_turn_stats`
   - IDs: turn, thread, task, goal, agent, provider profile, model alias, model.
   - Usage: input, non-cached input, cached input read/write, output,
     reasoning output, total, billable tokens, cost.
   - Latency: request duration, TTFT, TTFM, inference duration, stream duration.
   - Outcome: completed, failed, cancelled, no-action, final-submitted,
     max-steps, blocked reason.

2. `tool_call_stats`
   - IDs: call, turn, thread, task, goal, agent, tool descriptor, tool name.
   - Outcome: proposed, started, completed, failed, denied.
   - Timing: queued/start/end/duration.
   - Size: input bytes, output bytes, output lines, evidence count.
   - Governance: risk level, permission decision, security level, resource
     scope, builtin/MCP/imported origin.

3. `session_stats`
   - Totals by session/thread/task/goal/provider/model.
   - Cost, token totals, model requests, tool calls, failure counts, final
     submission status, artifact/evidence counts.
   - Context pressure and compaction counts.

4. `benchmark_case_stats`
   - Agent name, model, benchmark suite, instance ID, exit code, resolved flag,
     patch size, patch applies, tests passed/failed by bucket, wall time,
     retries/resumes, task bundle path, replay status.

5. `system_stats`
   - Event append count/duration, projection lag, state DB size, exported bundle
     size, audit log size, rollout bytes kept/dropped.

## Immediate Gaps To Close

- Extend provider usage to include cache read/write, reasoning output tokens,
  total tokens, provider request duration, and cost computed from model pricing.
- Debit tool-call and wall-time budgets where the tool/runtime paths actually
  spend them.
- Add a first-class stats projection in the store/runtime path. A CLI command
  should read that projection; it should not be the primary implementation of
  statistics.
- Record per-turn start/end timestamps and associate provider sessions and tool
  calls with the same turn ID.
- Make benchmark runner summaries include derived Agent-OS stats from the state
  DB, not only process-level paths and exit codes.
- Keep metric tags low-cardinality: provider, model alias, tool name, outcome,
  security level, benchmark suite, and agent role are useful; raw file paths,
  prompts, and command text belong in audit evidence, not metric tags.

## Store Design Direction

The forward design should add typed statistics state beside the current event
store, not outside it:

- Persist projection rows or projection snapshots in `agent-os-store-sqlite`
  for fast queries.
- Update projections from kernel state transitions in the same logical write
  path that appends events.
- Rebuild projections deterministically from current-schema events when the
  projection is missing or invalidated.
- Keep raw provider request/response payloads and high-cardinality command data
  in audit evidence, not metric tags or aggregate rows.
- Treat projection lag and rebuild status as system metrics.
