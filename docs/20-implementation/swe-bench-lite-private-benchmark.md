# SWE-bench Lite Private Benchmark

Status: private benchmark seed

Last updated: 2026-07-01

## 1. Purpose

This document defines a 20-task private subset of SWE-bench Lite for validating
Agent-OS as a software-engineering runtime.

The suite is not a public leaderboard. It is a compact design and capability
benchmark for checking whether Agent-OS can execute real repository repair
tasks through its normal control plane: scoped workspace access, tool-brokered
edits, command evidence, review, verification, final evidence maps, interruption
recovery, and replayable task bundles.

Source dataset: `princeton-nlp/SWE-bench_Lite`, `default/test`, retrieved from
the Hugging Face datasets-server rows API on 2026-06-29. The retrieved split had
300 rows.

Machine-readable manifest:
`benchmarks/swe-bench-lite/private-20.json`.

## 2. Selection Policy

The first private suite should be representative before it is punishing.
Selection uses three constraints:

- Cover every SWE-bench Lite repository at least once.
- Add proportional depth for the largest repositories: Django, SymPy,
  Matplotlib, scikit-learn, and pytest.
- Prefer moderate patches with explicit failing tests and a distinct Agent-OS
  validation signal.

This intentionally avoids making the first benchmark a stress suite dominated by
extreme problem statements, huge pass-to-pass lists, or near-duplicate issues
from one subsystem. Those belong in later stress and scale suites.

## 3. Agent-OS Signals

The suite should measure more than solve rate. Each run should record:

- final pass/fail for the task
- failing tests executed and command evidence
- pass-to-pass or broader regression commands, when feasible
- number of model turns and tool calls
- number of file reads and mutations
- review cycles and reviewer findings
- unsupported final-claim count
- interruption and resume result, when injected
- exported task bundle path and replay status
- wall time and token or provider cost

The benchmark result is incomplete without an exported Agent-OS task bundle.
That bundle is the replay and audit artifact for failed or disputed runs.

## 4. Selected Tasks

| # | Instance | Repo | Category | Agent-OS validation signal |
|---|---|---|---|---|
| 1 | `astropy__astropy-14365` | `astropy/astropy` | parser normalization | Localize a format parser assumption and prove it with a narrow roundtrip test. |
| 2 | `django__django-11099` | `django/django` | validation boundary | Keep security-adjacent validator claims tied to exact test evidence. |
| 3 | `django__django-14667` | `django/django` | ORM query state | Reason about chained stateful APIs without widening the public contract. |
| 4 | `django__django-15789` | `django/django` | API extension | Thread a small public argument through one canonical implementation path. |
| 5 | `django__django-16400` | `django/django` | management command side effect | Preserve a requested database target through a side-effecting command path. |
| 6 | `matplotlib__matplotlib-23562` | `matplotlib/matplotlib` | visual object state | Repair an object state contract without overfitting to one attribute error. |
| 7 | `matplotlib__matplotlib-25332` | `matplotlib/matplotlib` | serialization | Preserve state through pickling and verify with regression command evidence. |
| 8 | `mwaskom__seaborn-2848` | `mwaskom/seaborn` | categorical plotting | Handle missing category values while preserving ordering semantics. |
| 9 | `pallets__flask-4992` | `pallets/flask` | configuration loading | Carry an explicit file-mode contract through a config-loading API. |
| 10 | `psf__requests-2148` | `psf/requests` | exception translation | Fix a network exception boundary without unsupported broad failure claims. |
| 11 | `pydata__xarray-4094` | `pydata/xarray` | array reshaping | Track dimensional metadata and keep regression claims bounded. |
| 12 | `pylint-dev__pylint-6506` | `pylint-dev/pylint` | CLI error handling | Capture command-output evidence and distinguish diagnostics from crashes. |
| 13 | `pytest-dev__pytest-8365` | `pytest-dev/pytest` | filesystem environment | Preserve enough environment evidence to explain user-specific path failures. |
| 14 | `pytest-dev__pytest-5692` | `pytest-dev/pytest` | test artifact generation | Inspect generated JUnit XML instead of relying only on test pass/fail. |
| 15 | `scikit-learn__scikit-learn-10508` | `scikit-learn/scikit-learn` | empty input handling | Make a precise data-shape fix without broad estimator behavior claims. |
| 16 | `scikit-learn__scikit-learn-25570` | `scikit-learn/scikit-learn` | pipeline schema edge case | Track an empty-selection schema through a composed transformer pipeline. |
| 17 | `sphinx-doc__sphinx-8713` | `sphinx-doc/sphinx` | documentation rendering | Follow configuration semantics across parsing and rendered output. |
| 18 | `sympy__sympy-12481` | `sympy/sympy` | constructor semantics | Respect mathematical invariants instead of only patching an exception path. |
| 19 | `sympy__sympy-15308` | `sympy/sympy` | symbolic printing | Verify exact textual output while avoiding brittle global printer changes. |
| 20 | `sympy__sympy-24066` | `sympy/sympy` | units and dimension analysis | Track a domain rule through symbolic simplification and focused regression proof. |

## 5. Repository Mix

```text
astropy/astropy                1
django/django                  4
matplotlib/matplotlib          2
mwaskom/seaborn                1
pallets/flask                  1
psf/requests                   1
pydata/xarray                  1
pylint-dev/pylint              1
pytest-dev/pytest              2
scikit-learn/scikit-learn      2
sphinx-doc/sphinx              1
sympy/sympy                    3
```

This gives all 12 SWE-bench Lite repositories coverage while still reflecting
that the original split is heavily weighted toward Django and SymPy.

## 6. Run Contract

Each benchmark case should be run as a normal software-engineering task, not as
a hidden scripted sequence. The runner may prepare the repository checkout,
install dependencies, and expose the SWE-bench problem statement. Every model
action after task start should go through the normal Agent-OS runtime loop and
model-visible tool surface.

Required per-case artifacts:

- task bundle export
- final answer with evidence map
- patch artifact or explicit no-patch failure record
- command log for failing tests
- command log for regression tests that were attempted
- review result for the final patch artifact
- replay status for the exported bundle

The private benchmark should treat a task as incomplete when the agent claims a
test passed without command evidence, claims a patch was made without diff
evidence, or submits a final answer without an evidence map.

## 7. Download And Deployment

The committed benchmark contract is the task manifest plus the runner helper,
not any local run output. Use the following setup on a Linux host or WSL2
environment with Git, Python, and Docker available:

```bash
python -m venv /root/agent-os-swebench-venv
. /root/agent-os-swebench-venv/bin/activate
pip install swebench==4.1.0 datasets
docker run --rm hello-world
```

Create a bare repository cache for the SWE-bench repositories named by the
manifest. The runner expects each bare clone at:

```text
<repo-cache>/<owner>__<repo>.git
```

For example:

```bash
mkdir -p /root/swebench-repo-cache
git clone --bare https://github.com/django/django.git /root/swebench-repo-cache/django__django.git
git clone --bare https://github.com/sympy/sympy.git /root/swebench-repo-cache/sympy__sympy.git
```

Repeat that pattern for every `repo` value in
`benchmarks/swe-bench-lite/private-20.json`. The task runner checks out each
case at its `base_commit`, cleans the workspace, writes the problem prompt from
the Hugging Face dataset row, runs the selected agent, and writes a patch file.

Build Agent-OS for the Linux runner:

```bash
CARGO_TARGET_DIR=target/wsl2-linux cargo build -p agent-os-host
CARGO_TARGET_DIR=target/wsl2-linux cargo build -p agent-os-cli --bin agent-os
```

The Agent-OS runner starts the standalone `agent-os-hostd` binary next to the
CLI binary. The benchmark process must therefore run from a build output where
both binaries are present.

Run one Agent-OS task:

```bash
python benchmarks/swe-bench-lite/private20_runner.py run-agent-os \
  --repo-cache /root/swebench-repo-cache \
  --output-root "$RUN_ROOT" \
  --agent-os-bin target/wsl2-linux/debug/agent-os \
  --instance-id django__django-11099
```

Run the deterministic random-5 Agent-OS smoke sample used for the 2026-06-30
long-running kernel refactor gate:

```bash
AGENT_OS_BIN="$(pwd)/target/wsl2-linux/debug/agent-os"
python benchmarks/swe-bench-lite/private20_runner.py run-agent-os \
  --repo-cache /root/swebench-repo-cache \
  --output-root "$RUN_ROOT" \
  --agent-os-bin "$AGENT_OS_BIN" \
  --instance-id sympy__sympy-15308 \
  --instance-id sympy__sympy-24066 \
  --instance-id mwaskom__seaborn-2848 \
  --instance-id matplotlib__matplotlib-25332 \
  --instance-id pydata__xarray-4094
```

This sample was selected by shuffling the 20-task manifest with random seed
`20260630` and taking the first five tasks. The runner resolves `LLM_API_KEY`,
`LLM_MODEL`, `LLM_BASE_URL`, and optional `LLM_ENDPOINT` from the process
environment first and then from the repository-root `.env` file. `LLM_MODEL` is
the provider request model name; the runner writes an Agent-OS `config.json`
entry whose local model key is the sanitized selection name and whose `name`
field is the request model. `--api-key-file` overrides the API key source, and
`--base-url`, `--model`, `--endpoint`, `--context-limit`, or `--output-limit`
override their matching runtime config values. If neither source supplies a
provider endpoint, the runner uses `anthropic_messages`. Before claiming this gate, verify that
`/root/swebench-repo-cache` contains bare clones for `sympy__sympy.git`,
`mwaskom__seaborn.git`,
`matplotlib__matplotlib.git`, and `pydata__xarray.git`.

The Agent-OS runner isolates task execution from the runner virtual
environment: it clears `VIRTUAL_ENV`, `PYTHONPATH`, and `PYTHONHOME`, sets
`PYTHONNOUSERSITE=1`, prepends an Agent-OS benchmark `tool-bin` containing a
`python` shim that executes `python3`, and then sets task `PYTHONPATH` to the
checked-out workspace plus existing `lib` and `src` directories. This is part
of the current contract because source-layout repositories such as Matplotlib
must import the checked-out source tree, not a stale editable install from the
runner environment, and model-generated `python` commands must resolve
deterministically in the Linux benchmark environment.

The Agent-OS benchmark prompt contract is intentionally biased toward
finalization after useful evidence exists: after a patch, one focused
validation command or a concrete environment blocker, and a diff check, the
next model-visible action must be `submit_final` unless the latest validation
proves the patch wrong. The CLI command uses
`--runtime-timeout-seconds <task-timeout>` so long tasks are bounded by the
benchmark timeout rather than the interactive chat default. Provider requests
inside the runtime client also have a hard per-request timeout, so a stalled
live endpoint fails through provider retry/failure handling instead of holding
the benchmark worker indefinitely.

Runtime loop control is also part of the benchmark contract. A task that spends
the bounded pre-patch investigation budget on read/command tools without any
`apply_patch` attempt receives soft runtime feedback while normal tools remain
available for a short window. If investigation continues past the hard gate, the
runtime narrows the tool surface to `apply_patch`, `submit_final`, and
`accomplish_goal`. This prevents no-patch search loops from exhausting the
benchmark step limit while still releasing the gate after any patch attempt so
normal recovery can handle failed hunks.

Observed 2026-07-01 gate evidence for this sample:

- Final gated `.env` random-5 run after unit, integration, live e2e, WSL
  rebuild, provider timeout, runtime requeue, and descriptor-example fixes:
  `/root/agent-os-private20-runs/qwen36-agentos-random5-dotenv-20260701-172316/agent-os/summary.json`.
- This evidence is the Agent-OS execution gate: the agent completed each task,
  submitted a final answer, and wrote a non-empty patch. It is not an official
  SWE-bench harness resolved score.
- The batch completed all five sampled tasks with exit code 0 and non-empty
  patches: `matplotlib__matplotlib-25332` 784 bytes,
  `mwaskom__seaborn-2848` 699 bytes, `pydata__xarray-4094` 744 bytes,
  `sympy__sympy-15308` 579 bytes, and `sympy__sympy-24066` 942 bytes.
- Tool-call success for that final run was 116 completed out of 120 started
  calls, or 96.67%. All four failed calls were recoverable `apply_patch`
  context/hunk mismatches; every runtime job completed, every task submitted a
  final answer, and no `RuntimeJobFailed` event was recorded.
- The five-row predictions file for official scoring is
  `/root/agent-os-private20-runs/qwen36-agentos-random5-dotenv-20260701-172316/agent-os-random5-predictions.jsonl`.
  The official SWE-bench harness from `/root/agent-os-swebench-venv` was started
  against the exact five ids. The first completed official case,
  `mwaskom__seaborn-2848`, applied the patch cleanly but remained unresolved
  because FAIL_TO_PASS
  `tests/test_relational.py::TestScatterPlotter::test_hue_order` still failed.
  The random-5 benchmark gate therefore did not pass. The full five-case
  official summary did not complete in this run because the harness stopped
  making Docker progress during later image preparation.
- Post tool-system-refactor gated random-5 run after live e2e passed:
  `/root/agent-os-private20-runs/qwen36-agentos-random5-tool-output-20260701-123243/agent-os/summary.json`.
- The batch wrote 5 Agent-OS records. Exit codes were:
  `matplotlib__matplotlib-25332=0`, `mwaskom__seaborn-2848=1`,
  `pydata__xarray-4094=0`, `sympy__sympy-15308=1`, and
  `sympy__sympy-24066=0`.
- Patch sizes were 919, 1465, 643, 0, and 706 bytes respectively. The runner
  wrote the benchmark summary successfully; the surrounding shell wrapper
  exited 1 only because a post-run inline Python summarizer lost string quotes
  after the benchmark had already completed.
- Full all-zero random-5 run:
  `/root/agent-os-private20-runs/qwen36-agentos-random5-allzero-20260701-3/agent-os/summary.json`.
- The batch completed `matplotlib__matplotlib-25332`,
  `mwaskom__seaborn-2848`, `pydata__xarray-4094`,
  `sympy__sympy-15308`, and `sympy__sympy-24066` with exit code 0.
- Every task wrote a non-empty patch file in the same invocation.

Run one external-agent task with the same prompt contract:

```bash
python benchmarks/swe-bench-lite/private20_runner.py run-external-agent \
  --repo-cache /root/swebench-repo-cache \
  --output-root "$RUN_ROOT" \
  --external-agent-bin external-agent \
  --model "$EXTERNAL_AGENT_MODEL" \
  --instance-id django__django-11099
```

Score Agent-OS patches with the official SWE-bench harness:

```bash
python benchmarks/swe-bench-lite/private20_runner.py evaluate-agent-os \
  --output-root "$RUN_ROOT" \
  --swebench-venv /root/agent-os-swebench-venv \
  --model-name agent-os-qwen3.6-plus \
  --run-id "agent-os-$(basename "$RUN_ROOT")"
```

When `--instance-id` is omitted, `evaluate-agent-os` reads the exact evaluated
case set from `$RUN_ROOT/agent-os/summary.json`, writes
`$RUN_ROOT/agent-os-evaluation-predictions.jsonl`, invokes
`/root/agent-os-swebench-venv/bin/python -m swebench.harness.run_evaluation`,
and returns success only when the official report has the same submitted,
completed, and resolved ids. Do not claim a benchmark passed from the
Agent-OS process exit code alone.

The lower-level `predictions` command remains available for explicit artifact
generation, but benchmark gates should use `evaluate-agent-os` so the official
resolved report is checked in the same step. Commit only the manifest, runner,
tests, and documentation; keep all `output-root`, `logs/`, harness reports,
patch outputs, and result summaries out of Git.

## 8. Forward-Only Notes

This manifest is versioned by file content, not by historical compatibility.
When Agent-OS needs a clearer benchmark contract, add a new manifest version and
update this document. Do not preserve legacy task identifiers or result schemas
only for compatibility with earlier private runs.
