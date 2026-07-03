# Agent-OS Manifesto

Status: foundation

Last updated: 2026-06-25

## 0. Positioning

Agent-OS is not a stronger chat framework, nor is it a simple variant of AutoGen, LangGraph, or a workflow engine.

Its core position is:

```text
Build an agent-oriented runtime kernel on top of existing operating systems.
```

Traditional operating systems manage processes, threads, memory, files, permissions, networks, and devices. Agent-OS manages goals, context, memory, tools, permissions, evidence, risk, cost, human attention, and long-running task state.

Traditional operating systems assume the execution unit is a deterministic program. Agent-OS assumes the execution unit is a goal-driven, tool-using agent that operates under uncertainty.

## 1. One-Sentence Declaration

The goal of Agent-OS is to turn LLMs from intelligent functions into governable execution entities, and to turn agents from one-off conversation objects into observable, schedulable, collaborative, recoverable, auditable, and governable execution units.

It does not manage CPU threads as its primary abstraction. It manages Agent Threads and agent organizations.

It does not schedule CPU time. It schedules goals, context, tools, permissions, evidence, risk, cost, and human attention.

## 2. Core Problem

The biggest problem with current agents is not that they are completely unable to do work. The problem is:

```text
They can do some work, but they are unstable.
They can call tools, but they are hard to control.
They can generate results, but they are hard to audit.
They can complete short tasks, but struggle with long-running work.
They can launch multiple agents in parallel, but struggle to collaborate as an organization.
```

Agent-OS solves the problem of unmanageable AI labor.

It upgrades agents from:

```text
Prompt -> Model -> Tool Call -> Result
```

to:

```text
task registration
goal decomposition
permission assignment
context loading
agent scheduling
tool execution
evidence recording
result review
failure recovery
long-term memory
final delivery
```

This is not about making agents talk better. It is about making agents usable inside production systems.

## 3. Execution Unit: Agent Control Block

If an agent is the execution unit of an operating system, every agent must have a control block just as a process does.

A traditional operating system has a Process Control Block:

```text
pid
state
priority
registers
stack pointer
memory space
file descriptors
permissions
CPU time
```

Agent-OS should have an Agent Control Block:

```text
agent_id
parent_id
session_id
task_id
role
goal
state
context
memory
tools
permissions
budget
dependencies
risk_level
evidence
progress
artifacts
audit_log
reputation
```

The core state of a thread is its execution position. The core state of an agent is its goal, context, permissions, evidence, and progress.

## 4. Core Components

Agent-OS needs at least these kernel-level components:

```text
Scheduler and Resource Arbitration
Role and Profile System
Execution Environment System
Communication Kernel
Provider System
Context Manager
Memento Manager
Memory Manager
Tool Broker
Permission Kernel
Evidence Store
Artifact Store
Review Runtime
Conflict Resolver
Audit Log
```

Together, these components do one thing:

```text
They organize unstable intelligent calls into a stable workflow and auditable execution system.
```

## 5. Ten Fundamental Dimensions

Agent-OS must observe, schedule, and manage agents across the following ten dimensions.

### 5.1 Identity: Who It Is

The system must know:

```text
agent_id
agent_type
role
owner
parent_agent
spawned_by
session_id
task_id
```

Without identity, there is no audit, accountability, scheduling, or archival.

### 5.2 Goal: Why It Exists

An agent cannot rely only on implicit intent. It must explicitly hold its goal:

```text
global_goal
local_goal
current_subgoal
success_criteria
failure_criteria
deadline
```

A traditional operating system does not care what a thread wants to achieve. Agent-OS must care about an agent's goal; otherwise, it cannot know whether the agent has drifted off task.

### 5.3 State: Where It Is in Its Lifecycle

An agent lifecycle is not just running or blocked. It should include:

```text
Created
Ready
Planning
Thinking
CallingTool
WaitingTool
WaitingHuman
Blocked
Reviewing
Revising
Suspended
Completed
Failed
Quarantined
Terminated
```

An agent may be blocked because it lacks permission, is waiting for a tool, is waiting for human approval, lacks context, has an unclear goal, is waiting on a dependency, or is stuck in a loop. The system must distinguish these states.

### 5.4 Context: What It Currently Knows

Context is the agent's working memory. Agent-OS manages:

```text
context_id
context_size
context_summary
loaded_files
loaded_docs
visible_memory
context_freshness
context_pollution_score
```

Context must be loaded, isolated, compressed, cleaned, versioned, and rolled back. Traditional operating systems manage physical memory. Agent-OS manages semantic context.

### 5.5 Memory: Which Long-Term Memory It Can Access

More memory is not always better. Memory needs boundaries, sources, permissions, and versions:

```text
memory_namespace
readable_memory
writable_memory
project_memory
decision_log
experiment_log
bug_history
user_preference
```

Persisting wrong memory is more dangerous than having no memory.

### 5.6 Tools: What It Can Call

Tools are the system calls of agents. Agent-OS must manage:

```text
allowed_tools
denied_tools
tool_rate_limit
tool_risk_level
tool_confirmation_policy
```

If every agent can call every tool, the system has no organization. It is just a crowd of robots with root privileges.

### 5.7 Permission and Risk: What Consequences It Can Cause

Agent operations must be classified by risk:

```text
Level 0: read-only thinking
Level 1: read-only files and search
Level 2: local draft mutation
Level 3: code mutation
Level 4: command execution
Level 5: network and external API access
Level 6: production mutation, email sending, data deletion
```

Low-risk actions can run automatically. Medium-risk actions should be logged. High-risk actions require approval. Irreversible operations must involve a human in the loop.

### 5.8 Dependency: Who It Depends On and Who Depends On It

Agent-OS must maintain a task DAG:

```text
depends_on
blocks
input_artifacts
output_artifacts
required_evidence
```

Otherwise, a test agent may start before code is complete, a review agent may inspect an old diff, or multiple agents may modify the same file at the same time.

### 5.9 Evidence and Quality: Whether Its Output Is Trustworthy

Agent output should not flow directly into the final answer. Every important claim should carry:

```text
source_refs
tool_results
test_logs
diff_refs
confidence_per_claim
known_risks
unsupported_claim_count
```

Claims without evidence should not enter final output. High-risk claims must be externally verified.

### 5.10 Resource and Budget: What It Consumes

Agent-OS manages resources such as:

```text
token_budget
context_window
model_calls
tool_calls
wall_time
compute_cost
memory_reads
memory_writes
network_access
api_quota
human_attention_cost
```

Human attention is also a resource. Agents that frequently request low-value confirmation should be deprioritized.

## 6. Core Institutions

Agent-OS is not "a few more roles." It must establish institutions.

### 6.1 Mandatory Supervisor-Producer-Reviewer Loop

```text
SupervisorAgent decomposes the goal and assigns work.
ProducerAgent produces an artifact.
ReviewerAgent independently reviews the artifact and verifies evidence with tools.
SupervisorAgent accepts, rejects, delegates more work, or escalates.
```

Checks and balances do not mean agent debate. They mean different agents are accountable for different artifacts.

A separate verification role is not part of the foundation role set. Verification is a required ReviewerAgent mechanism.

### 6.2 Typed Blackboard

The system needs a structured blackboard, not just chat history:

```json
{
  "goal": "...",
  "constraints": [],
  "known_facts": [],
  "hypotheses": [],
  "decisions": [],
  "open_questions": [],
  "tasks": [],
  "risks": [],
  "test_results": [],
  "artifacts": [],
  "final_acceptance_criteria": []
}
```

The blackboard is the shared state of organizational collaboration and the foundation for audit and recovery.

### 6.3 Evidence-First Final Answer

Final answers must cite real evidence:

```text
which files changed
what the diff is
which tests ran
what the test results were
which risks remain
which conclusions were not verified
```

Natural-language summaries must be generated from evidence, not used as substitutes for evidence.

### 6.4 Separation of Permission and Responsibility

Different agents should have different permissions:

```text
SupervisorAgent: can read, write, run commands, assign child agents, arbitrate permissions, and accept final results under policy.
ProducerAgent: can read, write, run commands, and produce evidence-backed artifacts under scoped policy, but cannot create child agents or accept its own artifact.
ReviewerAgent: has equivalent baseline capability to ProducerAgent, but must use it for independent review and evidence verification; it cannot mutate the artifact under review or accept final results.
```

Organization exists only when responsibility boundaries are clear.

## 7. What It Solves

The most important value of Agent-OS appears in ten areas:

```text
1. long-task instability
2. multi-agent organization instead of group chat
3. permission and safety
4. context management
5. evidence and auditability
6. failure recovery
7. cost scheduling
8. concurrency and conflict
9. human attention waste
10. organizational memory
```

The first breakout domains may be software engineering, automated testing, scientific experimentation, enterprise office automation, security operations, and SRE.

## 8. What It Cannot Solve

Agent-OS cannot magically solve:

```text
insufficient model reasoning ability
missing knowledge in training data
lack of verifiable real-world signals
unreliable tool interfaces
unclear user goals
tasks without evaluation criteria
```

The model determines the capability ceiling of a single agent. Agent-OS determines whether many agents can form stable productivity.

## 9. Final Vision

Agent-OS does not replace Linux, Windows, or macOS. It replaces today's messy mix of prompts, scripts, workflows, RPA, bots, CI glue code, and manual coordination.

It moves computer interaction from:

```text
human opens software -> clicks buttons -> watches pages
```

to:

```text
human states a goal -> Agent-OS assigns agents -> execution -> verification -> report -> wait for necessary approval
```

This is the platform-level meaning of Agent-OS:

```text
Move computing from "humans operate software" to "humans manage agent organizations."
```

## 10. Source

This document is based on a shared conversation:

<https://chatgpt.com/share/6a3ce438-ffc0-83ec-a50f-0731365caa5d>
