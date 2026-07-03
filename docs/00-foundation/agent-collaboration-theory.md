# Core Theory of Agent Collaboration

Status: foundation

Last updated: 2026-07-01

## 0. Core Thesis

The essence of multi-agent collaboration is not group chat. It is organization.

Effective agent collaboration is not about multiple agents debating, supplementing each other, or voting. It is about establishing an executable collaboration institution:

```text
clear division of labor
parallel progress
context isolation
permission separation
shared state
evidence recording
independent review
tool-based verification
conflict arbitration
final acceptance
```

In one sentence:

```text
The core value of multiple agents is not "more opinions"; it is turning complex work into a manageable organizational process.
```

## 1. Five Root Mechanisms

Agent collaboration can be compressed into five root mechanisms:

```text
1. division of labor
2. parallelism
3. checks and balances
4. shared blackboard
5. permission and responsibility boundaries
```

These five mechanisms are more fundamental than "role play," "expert debate," or "collective intelligence," and they are more suitable for engineering.

## 2. Division of Labor: Not Role Play, but Isolation of Context, Tools, and Permissions

Many multi-agent systems fail because they only give agents different names:

```text
architect
programmer
tester
critic
judge
```

If those agents have the same context, tools, permissions, and goal, this is not real division of labor. It is just several prompt variants.

Real division of labor should appear in five layers:

```text
different goals
different context
different tools
different permissions
different artifacts
```

The core organizational roles are:

```text
SupervisorAgent: owns goals, task DAGs, scheduling, delegation, permission arbitration, final acceptance, and risk control
ProducerAgent: produces artifacts such as plans, patches, test logs, research notes, experiments, or reports
ReviewerAgent: independently reviews artifacts, checks assumptions, and verifies evidence with tools
```

Planner, Explorer, Researcher, Coder, Tester, and Reporter are specializations or profile variants under ProducerAgent or ReviewerAgent. They are not separate foundation roles.

The key to division of labor is not "who sounds more expert." It is "who is responsible for which artifact."

## 3. Parallelism: Not Simultaneous Chat, but Isolation of Noisy Work

The value of parallelism comes from three sources:

```text
reducing pollution in the main context
increasing exploration speed
allowing independent work to progress simultaneously
```

Tasks suitable for parallelism include:

```text
reading different modules
checking different documents
analyzing different logs
running different tests
generating different candidate plans
reviewing different risk surfaces
```

Tasks unsuitable for parallelism include:

```text
modifying the same file at the same time
writing tests before the design is stable
summarizing before dependencies are complete
having multiple agents write the same memory simultaneously
```

Parallelism must therefore be bound to dependency management, artifact management, and locking.

More agents in parallel is not automatically better. Without a dependency graph and merge mechanism, parallelism becomes a token sink.

## 4. Checks and Balances: Not Debate, but the Producer-Reviewer Loop

Debate is one of the most overvalued mechanisms in agent collaboration.

Pure debate has several problems:

```text
no external verification
the most persuasive agent can win
implementation-level defects are not found reliably
limited usefulness for code, testing, and experimentation
```

A stronger mechanism is the checks-and-balances loop:

```text
Producer creates an artifact.
Reviewer independently reviews it and verifies evidence with tools.
Supervisor arbitrates based on evidence.
```

In software engineering:

```text
Coder writes a patch.
Tester runs tests.
Reviewer inspects the diff and checks logs, builds, tests, and benchmarks.
Supervisor decides whether acceptance criteria are satisfied.
```

In scientific experimentation:

```text
Hypothesis Agent proposes a hypothesis.
Paper Agent checks the literature.
Implementation Agent implements the experiment.
Experiment Agent runs the experiment.
Analysis Agent analyzes the result.
Reviewer Agent checks whether conclusions are supported by data and tool evidence.
```

Core rules:

```text
The producer cannot be the sole acceptor of its own artifact.
The reviewer has equivalent baseline capability to the producer, but a different responsibility contract.
The reviewer must not mutate the artifact under review while reviewing it.
The reviewer should prefer tools over natural-language judgment when evidence can be checked.
Conflicting conclusions enter the conflict resolver.
The final answer must be evidence-first.
```

## 5. Shared Blackboard: Organizational Collaboration Needs Structured Shared State

Conversation history alone is not enough for multi-agent collaboration.

The problems with conversation history are:

```text
unstable structure
hard to query
hard to audit
hard to know which facts are still valid
hard to express task dependencies and artifact state
```

A multi-agent system needs a typed blackboard:

```json
{
  "goal": "...",
  "constraints": [],
  "known_facts": [],
  "hypotheses": [],
  "decisions": [],
  "open_questions": [],
  "tasks": [
    {
      "id": "T1",
      "owner": "Explorer",
      "status": "done",
      "evidence": "..."
    }
  ],
  "risks": [],
  "artifacts": [],
  "test_results": [],
  "review_results": [],
  "final_acceptance_criteria": []
}
```

The blackboard is not a UI feature. It is a combination of runtime, database, schema, and workflow.

Its functions are:

```text
let each agent know the current organizational facts
keep the main agent from being polluted by noise
allow tasks to recover
make evidence traceable
make conflicts discoverable
give final delivery a source
```

## 6. Permission and Responsibility Boundaries: No Boundaries, No Organization

If every agent can read everything, edit every file, run every command, and access every network, the system has no collaboration structure. It only has multiple high-privilege risk points.

Permissions should be configured by role:

```text
SupervisorAgent:
  read: allow
  edit: allow
  bash: allow under policy
  network: policy gated
  child agents: SupervisorAgent, ProducerAgent, ReviewerAgent
  final acceptance: allow

ProducerAgent:
  read: allow
  edit: workspace scoped
  bash: restricted by risk and permission profile
  network: deny by default or policy gated
  child agents: deny
  final acceptance: deny for own artifact

ReviewerAgent:
  read: allow
  edit: workspace scoped capability, but forbidden for the artifact currently under review
  bash: restricted by risk and permission profile
  network: deny by default or policy gated
  child agents: deny
  final acceptance: deny
```

Permission boundaries are also responsibility boundaries.

Whatever an agent can do must be traceable: why it did it, when it did it, what it affected, and whether it can be rolled back.

## 7. Minimum Closed Loop for Agent Collaboration

A usable agent collaboration system needs this minimum loop:

```text
1. Register Goal
2. Define Acceptance Criteria
3. Build Task DAG
4. Assign Agents
5. Load Scoped Context
6. Execute Tools
7. Submit Artifacts
8. Record Evidence
9. Review Independently with Tool Verification
10. Resolve Conflicts
11. Produce Final Answer
12. Update Memory
```

If any part is missing, the system degenerates:

```text
no acceptance criteria -> no way to know when work is complete
no task DAG -> parallel work tramples itself
no evidence -> conclusions are not auditable
no evidence-backed review -> errors flow directly into final output
no memory -> every task starts from zero
no permissions -> agents can easily lose control
```

## 8. Structured Communication: Agent IPC

Agents should not communicate only through natural-language chat. They should use structured messages.

Common message types:

```text
ContextRequest
ReviewRequest
TestRequest
FindingReport
BlockerReport
ArtifactSubmitted
ConflictRaised
ApprovalRequest
DecisionCommitted
```

A message can look like:

```json
{
  "type": "ReviewRequest",
  "from": "ProducerAgent",
  "to": "ReviewerAgent",
  "artifact": "patch-123",
  "focus": ["correctness", "edge cases", "performance"],
  "required_evidence": ["diff", "test log"],
  "deadline": "10min",
  "priority": "high"
}
```

This corresponds to IPC in a traditional operating system.

Agent collaboration needs a protocol, not casual chat.

## 9. Scheduling Principles

Agent scheduling cannot consider only who is idle, nor can it consider only task priority.

It should combine:

```text
priority
deadline
risk_level
cost_budget
dependency_ready
expected_value
uncertainty
resource_need
tool_availability
context_locality
human_attention_cost
preemption_cost
progress_score
```

Basic rules:

```text
high priority + low risk + dependencies ready -> run first
high risk + no approval -> block
high cost + low value -> downgrade or suspend
long time without progress -> kill / compact / replan
severe context pollution -> compact / reset / split
frequent user interruption -> downgrade or merge questions
```

Agent scheduling is better suited to cooperative scheduling. Do not preempt in the middle of LLM reasoning. Schedule at boundaries:

```text
before LLM call
after LLM call
before tool call
after tool call
before artifact commit
before approval request
during task stage transition
```

## 10. Conflict Handling

Multi-agent collaboration inevitably creates conflicts:

```text
two agents provide opposite conclusions
reviewer rejects coder implementation
tester finds implementation failure
researcher's external source disagrees with local code
two agents modify the same file at the same time
```

Conflicts should not be resolved by "who sounds more reasonable." They should enter the conflict resolver:

```text
first prefer real tool results
then project rules
then explicit user preferences
then historical decisions
finally human arbitration
```

Conflict-handling principles:

```text
test results outrank natural-language confidence
current code facts outrank memory summaries
current files outrank old conclusions
explicit user instructions outrank agent inference
unverifiable conclusions must be marked uncertain
```

## 11. Artifact Management

Agent collaboration is not just text exchange. It is artifact submission.

Every artifact should be tracked:

```text
artifact_id
artifact_type
owner_agent
version
status
diff
provenance
reviewed_by
test_result
rollback_point
```

Typical artifacts include:

```text
plan
patch
test log
benchmark result
review report
bug report
experiment config
analysis note
final answer
memory update
```

If the system does not know what an agent generated, who reviewed it, whether it was verified, and whether it can be rolled back, multi-agent collaboration cannot enter production.

## 12. Failure Modes

Common failure modes in agent collaboration:

```text
1. group-chat degeneration: multiple agents only express opinions
2. hollow roles: different names, same context and permissions
3. evidence-free summary: final output only synthesizes statements
4. context pollution: logs, old plans, and wrong hypotheses mix together
5. parallel trampling: multiple agents modify the same resource
6. review failure: reviewer lacks independent context or read-only constraints
7. tool mismatch: guessing when verification is needed, or running commands when thinking is needed
8. overbroad permissions: every agent can write files and run commands
9. memory pollution: wrong conclusions become long-term memory
10. user interruption overload: low-value confirmations consume human attention
```

These problems cannot be fully solved by prompts. They require system mechanisms.

## 13. Minimum Viable Collaboration Architecture

A minimum viable agent collaboration architecture can be designed as:

```text
SupervisorAgent
  - owns goal
  - owns task DAG
  - owns blackboard
  - assigns SupervisorAgent, ProducerAgent, or ReviewerAgent children
  - resolves conflicts
  - produces final

ProducerAgent
  - produces artifacts
  - may be specialized as Planner, Explorer, Researcher, Coder, Tester, or Reporter
  - reports progress, blockers, evidence, and artifacts to SupervisorAgent

ReviewerAgent
  - independently reviews artifacts
  - verifies evidence with tools
  - may be specialized by review domain, but remains separate from the producer of the artifact

Runtime Services
  - Context Manager
  - Permission Kernel
  - Tool Broker
  - Artifact Store
  - Evidence Store
  - Memory Store
  - Audit Log
```

Minimum institutions:

```text
every patch must have a diff
every final answer must have evidence
every high-risk operation must require approval
every long-term memory write must have a source
every review must be independent from the coder
every parallel mutation must have a lock
```

## 14. Final Theory

The real theory of agent collaboration is not "multiple intelligent agents produce collective intelligence."

More precisely:

```text
Agent collaboration = goal decomposition + context isolation + permission separation + artifact flow + evidence constraints + independent review + tool verification + organizational memory.
```

The value of multiple agents is not that more agents speak at once. It is that an unstable intelligent process is decomposed into observable, verifiable, recoverable organizational steps.

The final goal is not to make agents behave like a group of people in a meeting. It is to make agents work like an engineering organization.

## 15. Source

This document is based on a shared conversation:

<https://chatgpt.com/share/6a3ce438-ffc0-83ec-a50f-0731365caa5d>
