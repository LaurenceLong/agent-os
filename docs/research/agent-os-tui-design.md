# Agent-OS TUI 与启动命令技术方案

## Summary

- 架构边界固定为：`kernel` 持有真相，`thread` 负责 LLM runtime loop，`host` 负责本地 orchestration，`app protocol` 是客户端边界，`cli/tui` 只负责 presentation。
- 新增 `agent-os-tui` 作为交互式外壳，支持像 Codex/OpenCode 一样直接发消息给 LLM，并通过 runtime/tool loop 执行工具。
- TUI 内部保留丰富 slash/palette 命令体系；这些命令是一等交互入口，但不是公共协议。每个命令最终映射到 `AppRequest`、本地 UI state，或只读 projection。
- `agent-os` 默认进入 TUI；`agent-os run/chat/status/process/resume` 保留为脚本化、CI、fallback 和机器可读入口。

## Architecture

```text
agent-os-sys
  AppRequest/AppNotification、projection types、ABI/data schema

agent-os-kernel
  权威状态机：thread/task、permission、tool、evidence、artifact、replay

agent-os-thread
  LLM runtime：ModelTurnRequest、ModelAction、ToolCall -> kernel syscall

agent-os-host
  本地 control plane：持有 Kernel，管理 runtime jobs/workers，处理 AppRequest，生成 projection/notification

agent-os-app-server
  transport/protocol：JSONL envelope、initialize、subscribe、JsonlAppClient/AppServer

agent-os-cli
  脚本化外壳：flags/subcommands -> AppRequest，输出 JSON/文本

agent-os-tui
  交互式外壳：composer、slash、palette、picker、approval、timeline、overlay
```

关键边界：

- TUI 不直接依赖 `Kernel` 或 `ThreadRuntime`。
- CLI/TUI/Desktop 都通过 `agent-os-hostd` 或 in-process `AgentOsHost + AppServer` 进入系统。
- 公共客户端 contract 是 `AppRequest/AppNotification`，不是 CLI flags，也不是 TUI slash。
- TUI slash/palette 是 TUI 内部 command registry，文档可由 registry 生成。

## Startup Commands

- 默认交互入口：

```bash
agent-os [WORKSPACE]
agent-os tui [WORKSPACE]
```

行为：

- 无子命令时默认启动 TUI。
- `WORKSPACE` 省略时使用当前目录。
- `agent-os tui` 与默认入口等价，适合文档和显式调用。

- TUI 参数：

```bash
agent-os tui [WORKSPACE]
  --thread <id>
  --resume <thread-or-job-id>
  --model <alias>
  --profile <name>
  --state-db <path>
  --max-steps <n>
  --max-tokens <n>
  --temperature <value>
  --no-alt-screen
```

- 脚本/CI/fallback 入口：

```bash
agent-os run [PROMPT...]
agent-os chat
agent-os status [thread-id]
agent-os resume <thread-or-job-id>
agent-os process list|stop|kill
```

- host 高级入口：

```bash
agent-os-hostd --stdio --state-db <path> --model <alias>
agent-os host serve --stdio
```

`agent-os host serve` 只作为高级调试包装；普通用户文档不引导直接使用 hostd。

## TUI Command Registry

- Registry 是 TUI 内部唯一命令源，同时驱动：
  - slash autocomplete
  - command palette
  - keymap hints
  - help overlay
  - context availability filtering
- 每条命令包含：
  - command id
  - slash name
  - aliases
  - title/description/category
  - inline args 支持
  - running-turn 可用性
  - side/scratch 可用性
  - handler target: `AppRequest`、UI-only、projection-only

### Conversation / Runtime

```text
/new
  清空当前 composer，进入新 thread 准备状态。

/run [prompt]
  无参数时提交 composer；有参数时直接作为普通用户消息提交给 LLM。

/interrupt
  中断当前 running turn，映射 TurnInterrupt。

/steer <message>
  running turn 中追加 steering input，映射 TurnSteer。

/status
  打开当前 thread/job/model/workspace/risk 状态面板。

/raw on|off
  切换 raw timeline/projection 显示。

/copy
  复制最后一条 agent final/message 或当前选中 timeline item。

/clear
  清空 TUI 当前 scrollback，不删除 thread 或 kernel state。

/exit
/quit
  退出 TUI。
```

### Thread / Session Lifecycle

```text
/threads
/sessions
  打开 thread picker，支持搜索、恢复、归档状态过滤。

/resume <thread-or-job-id>
  恢复指定 thread 或 runtime job。

/fork [turn-id]
  fork 当前 thread；无参数时从当前选中 turn 或 latest turn fork。

/rollback [turn-id|item-id|event-id]
  打开 rollback picker 或按 inline target rollback。

/rename <title>
  重命名当前 thread。

/archive
  归档当前 thread 并回到 thread picker。

/delete
  删除当前 thread，需 bottom pane 确认。

/compact
  请求 context compaction；无参数时由 UI 根据 projection 给出默认 superseded refs。
```

### Model / Provider / Profile

```text
/model [alias]
/models
  打开 model picker 或切换模型 alias，影响后续 turn/start 配置。

/provider
/providers
  查看 provider/capability 状态。

/profile [name]
  切换或查看 profile；v1 只影响新 thread 或下一次 host config，避免修改 running turn。

/usage
  打开 provider usage/stats 面板。
```

### Permissions / Tools / Processes

```text
/permissions
  打开 permission profile/risk ceiling 面板。

/approve
  打开 pending approvals 面板。

/tools
  查看当前 model-visible tool inventory 和 deferred tool 状态。

/mcp
  查看 MCP tools/resources/templates projection。

/processes
/ps
  查看运行中的 process sessions。

/stop <process-id>
  停止 process，映射 ProcessStop。

/kill <process-id>
  kill process，映射 ProcessKill。
```

### Context / Evidence / Debug Inspection

```text
/context
  打开 model context projection overlay。

/events
  打开 app/kernel event timeline overlay。

/replay
  打开 replay transcript overlay。

/evidence
  查看 evidence index 和 final evidence map。

/artifacts
  查看 artifact index。

/diff
  打开 workspace diff/resource view。

/debug
  打开 debug projection，包括 runtime_jobs、resources、automation_runs。
```

### Goal / Plan / Side Interaction

```text
/goal [clear|pause|resume|<objective>]
  查看或调整当前 goal intent；能映射到已有 app/kernel 能力时走 AppRequest，否则 v1 先做 UI-only draft。

/plan [prompt]
  进入 planning-style user message；本质仍是普通 user input，但 UI 标记为 planning intent。

/side <message>
/scratch <message>
  发起 scoped scratch/side interaction；v1 可先作为 UI-only side composer，不改变 kernel contract，直到后续有正式 thread side-channel。
```

### Help / Configuration

```text
/help
  打开由 registry 生成的帮助。

/keymap
  打开 keymap overlay。

/theme
  打开 TUI theme picker。

/editor
  用外部 EDITOR 编辑当前 composer 内容。
```

## Implementation Changes

- Shared host connection
  - 保留 `agent-os-app-server::JsonlAppClient` 作为协议客户端。
  - 在 `agent-os-host` 新增 public `StdioHostConfig` / `StdioHostClient` / hostd resolution API。
  - 将 `agent-os-cli::support` 中现有私有 hostd 启动逻辑迁移到该 public API，CLI 和 TUI 共用。
  - 该层只负责进程生命周期、请求、订阅和通知读取，不定义业务动作模型。

- CLI reshape
  - 更新 `agent-os-cli` dispatch：无参数或首参数是路径时启动 TUI；`help/-h/--help` 仍显示帮助。
  - 添加 `tui` 子命令，调用 `agent-os-tui::run_tui`.
  - 保留 `run/chat/status/process/resume/code`，全部继续走 app protocol。
  - `chat` 作为行式 fallback，不作为主交互入口宣传。

- TUI crate
  - 新增 workspace member `crates/agent-os-tui`.
  - 使用 `ratatui` + `crossterm`.
  - 暴露 `run_tui(TuiOptions) -> AgentOsResult<TuiExitReport>`.
  - 内部模块：`app`、`composer`、`command_registry`、`keymap`、`timeline`、`bottom_pane`、`overlay`、`projection`。
  - TUI 启动时创建 `StdioHostClient`，发送 `Initialize`，然后 `Subscribe`.

- TUI message/runtime flow
  - 普通 composer 文本：
    - 无当前 thread：`ThreadStart { goal, workspace }` 后 `TurnStart { client_thread_id, input }`。
    - 有 ready thread：`TurnStart { client_thread_id, input }`。
    - 有 running turn：`TurnSteer { turn_id, input }`。
  - TUI 使用 `AppNotification` 增量更新 timeline，并用节流 `ThreadRead` 补齐投影。
  - LLM 工具调用仍由 `agent-os-thread` 通过 kernel 执行；TUI 只展示 tool proposal/result 和 approval UI。

- TUI UI surfaces
  - 主屏：顶部 status bar、中部 timeline、可切换 inspector、底部 composer。
  - Composer popup：slash、file/context mention、command palette search。
  - Bottom pane：approval、thread picker、model picker、permission/status 面板。
  - Full overlay：events、context projection、tool inventory、evidence/artifact、diff、help/keymap。
  - Mode stack 优先级：overlay > bottom pane > composer popup > composer。
  - `Esc` 关闭当前 mode；`Ctrl-C` running 时触发 interrupt 确认，非 running 且空 composer 时退出确认。

## Test Plan

- Unit tests
  - CLI dispatch：无参数进入 TUI；`help` 不启动 TUI；已有子命令行为保持。
  - host client：构造 hostd 命令参数、client identity、request/rejected response 映射。
  - command registry：slash 解析、alias、inline args、running 状态可用性、help 生成。
  - keymap：command id 绑定、冲突检测、mode-specific dispatch。
  - TUI reducer：处理 `ThreadChanged`、`AgentMessageDelta`、`ToolUpdate`、`ApprovalRequested`、重复通知和补读。

- Integration tests
  - fake app client 驱动 TUI state：普通文本提交产生 `ThreadStart` + `TurnStart`。
  - running turn 中提交普通文本产生 `TurnSteer`。
  - `/interrupt` 产生 `TurnInterrupt`。
  - `/fork`、`/rollback`、`/compact` 产生对应 thread AppRequest。
  - approval bottom pane 调用 `ApprovalRespond`。
  - thread picker 调用 `ThreadList`/`ThreadRead` 并切换当前 thread。
  - `/tools`、`/context`、`/evidence` 只读 projection，不产生 mutation request。

- Regression tests
  - 现有 `agent-os chat/run/resume/status/process` 继续通过。
  - `cargo test -p agent-os-cli -p agent-os-host -p agent-os-app-server -p agent-os-tui`.
  - 变更完成后运行 `cargo clippy --workspace --all-targets -- -D warnings`。

- Live LLM e2e
  - 如果实现只新增 TUI/CLI 外壳和 host client helper，不改变 `agent-os-thread` runtime、provider request、tool schema 或 model-visible context，则不新增 live LLM e2e。
  - 如果改动触及 runtime loop、prompt、tool visibility、provider adapter 或 final submission，则按项目规则运行对应 ignored live test 并记录 provider、命令、结果和 audit log。

## Assumptions

- v1 只做 full-screen TUI；mini/split-footer 暂不实现。
- v1 不新增 socket daemon 管理；默认仍用 stdio hostd。
- `agent-os-host` 是 hostd 启动 helper 的归属地；`agent-os-app-server` 继续只关心 JSONL protocol。
- `agent-os` 默认 TUI 是 forward-only CLI contract 变更；旧的“无参数显示 usage”不保留。
- TUI slash/palette 是稳定的 TUI UX contract，但不是跨客户端公共 API；跨客户端公共 API 仍然只有 app protocol。
