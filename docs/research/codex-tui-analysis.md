# Codex TUI 调研报告

调研目标：分析 `D:\work\ai\codex` 中 Codex TUI 启动后支持的用户命令、slash commands、快捷键、overlay/picker/control surfaces，以及这些交互如何映射到源码模块。  
调研方式：只读 Codex 源码；本文件是本次任务唯一写入产物。

## 1. CLI 启动命令

Codex TUI 有两层入口：顶层 `codex` CLI 和 TUI crate 自身入口。

### 1.1 顶层 `codex` CLI

顶层入口位于：

- `D:\work\ai\codex\codex-rs\cli\src\main.rs`

关键结构：

- `MultitoolCli`：顶层 CLI 解析结构，定义 `codex` 的全局参数和子命令。
- 无子命令时进入交互式 TUI，路径大致为 `cli_main -> run_interactive_tui -> codex_tui::run_main`。
- 顶层还提供 `exec`、`review`、`login`、`logout`、`mcp`、`plugin`、`app-server`、`remote-control`、`completion`、`doctor`、`sandbox`、`resume`、`archive`、`delete`、`unarchive`、`fork`、`cloud`、`features` 等命令。
- `--remote` 和 `--remote-auth-token-env` 在顶层 interactive remote options 中定义，用于连接远程 app-server。
- `--enable` / `--disable` feature toggles 在顶层解析后合并到 config overrides，再传入 TUI。

### 1.2 TUI crate 入口

TUI crate 入口位于：

- `D:\work\ai\codex\codex-rs\tui\src\main.rs`

关键流程：

- `TopCli` flatten 根级 `CliConfigOverrides` 和 `codex_tui::Cli`。
- `main` 解析参数后，把根级 config overrides 合并进 TUI CLI。
- 最终调用 `codex_tui::run_main(inner, arg0_paths, LoaderOverrides::default(), None)`。

### 1.3 TUI 参数定义

TUI 参数定义位于：

- `D:\work\ai\codex\codex-rs\tui\src\cli.rs`
- `D:\work\ai\codex\codex-rs\utils\cli\src\shared_options.rs`

TUI 自身参数：

- `PROMPT`：启动时的初始用户提示。
- `--strict-config`：遇到不识别的 config 字段时报错。
- `--ask-for-approval` / `-a`：设置模型执行命令前的人类审批策略。
- `--search`：启用 live web search，并在 `run_main` 中转为 `web_search = "live"` 配置。
- `--no-alt-screen`：禁用 alternate screen，保留终端 scrollback。
- 内部字段：`resume_picker`、`resume_last`、`resume_session_id`、`resume_show_all`、`resume_include_non_interactive`、`fork_picker`、`fork_last`、`fork_session_id`、`fork_show_all`。这些由顶层 `codex resume` / `codex fork` 包装命令设置，不作为基础 `codex` 命令公开参数。

共享参数：

- `--image` / `-i FILE`：初始 prompt 附带图片。
- `--model` / `-m`：选择模型。
- `--oss`：使用开源 provider。
- `--local-provider`：指定 `lmstudio` 或 `ollama`。
- `--profile` / `-p`：加载 `$CODEX_HOME/<name>.config.toml` profile。
- `--sandbox` / `-s`：选择 sandbox 策略。
- `--dangerously-bypass-approvals-and-sandbox` / `--yolo`：跳过审批并禁用 sandbox。
- `--dangerously-bypass-hook-trust`：本次运行绕过 hook trust。
- `--cd` / `-C DIR`：指定工作目录。
- `--add-dir DIR`：额外授予可写目录。

### 1.4 TUI 初始化主流程

主流程位于：

- `D:\work\ai\codex\codex-rs\tui\src\lib.rs`

关键职责：

- 根据 CLI 和 config 构造 `ConfigOverrides`。
- 将 `--dangerously-bypass-approvals-and-sandbox` 映射为 `SandboxMode::DangerFullAccess` 和 `AskForApproval::Never`。
- 将 legacy `--search` 转为 `web_search = "live"`。
- 解析 `codex_home`、profile v2、cwd、额外 writable roots。
- 处理本地 app-server、remote app-server、embedded app-server 的连接方式。
- 处理 OSS provider、默认模型、登录/onboarding/trust、state DB 初始化。
- 进入 ratatui app 运行循环。

## 2. TUI 架构

Codex TUI 的交互架构可以分为几层。

### 2.1 App 事件层

关键路径：

- `D:\work\ai\codex\codex-rs\tui\src\app_event.rs`
- `D:\work\ai\codex\codex-rs\tui\src\app.rs`
- `D:\work\ai\codex\codex-rs\tui\src\app\input.rs`

`AppEvent` 是 TUI 内部控制面的事件总线。slash command、快捷键、approval、picker、overlay、runtime 回调都会转成 app event 或直接驱动 bottom pane view。

典型事件包括：

- `OpenAgentPicker`
- `OpenResumePicker`
- `OpenReasoningPopup`
- `OpenAllModelsPopup`
- `OpenFullAccessConfirmation`
- `OpenWorldWritableWarningConfirmation`
- `OpenApprovalsPopup`
- `OpenManageSkillsPopup`
- `OpenPermissionsPopup`
- `OpenReviewBranchPicker`
- `OpenReviewCommitPicker`
- `FullScreenApprovalRequest`
- `LaunchExternalEditor`

### 2.2 ChatWidget 层

关键路径：

- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\interaction.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\skills.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\connectors.rs`

职责：

- 处理 chat 区域和 composer 附近的键盘事件。
- 管理 `Ctrl-C` 中断/退出、`Ctrl-D` 退出、图片粘贴、reasoning effort 调整、queued message 编辑。
- 接收 slash command dispatch，并转成本地 UI 操作、runtime submission、app event 或 output item。
- 承载 skills、connectors/apps、plugins、MCP/status 等功能入口。

### 2.3 Composer 和 popup 层

关键路径：

- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer\slash_input.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer\popup_state.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\command_popup.rs`

职责：

- 编辑用户输入。
- 检测 slash command、file mention、skill mention、mention v2。
- 同一时间只允许一个 active popup：`Command`、`File`、`Skill`、`MentionV2` 或 `None`。
- 区分 bare slash command、inline slash command、queued slash command、shell command。
- 支持 Vim normal mode 下 `/` 进入 slash command popup，`!` 进入 shell/bash 输入模式。

### 2.4 Bottom pane view 层

关键路径：

- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\bottom_pane_view.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\approval_overlay.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\list_selection_view.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\selection_list.rs`

职责：

- 为 approval、picker、form、setup flow 提供统一 trait。
- 统一处理键盘事件、完成态、Esc 行为、Ctrl-C、paste、pre-draw tick、request consumption。
- 复用 list selection 组件实现 model picker、permissions picker、resume picker、review picker 等。

### 2.5 Overlay 层

关键路径：

- `D:\work\ai\codex\codex-rs\tui\src\pager_overlay.rs`

职责：

- 使用 alternate screen 展示长内容。
- 支持 transcript overlay 和 static overlay。
- 支持 pager keymap：上下滚动、PageUp/PageDown、Home/End、关闭等。

## 3. Slash commands 完整清单

权威定义：

- `D:\work\ai\codex\codex-rs\tui\src\slash_command.rs`

过滤和 popup：

- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\slash_commands.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\command_popup.rs`

执行分派：

- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`

### 3.1 command registry 设计

`SlashCommand` enum 同时承载：

- canonical command name。
- alias。
- popup 展示描述。
- 是否支持 inline args。
- 是否可在 side conversation 中使用。
- 是否可在 task running 时使用。
- 平台/构建条件可见性，例如 Windows sandbox command、macOS/Windows Desktop app command、debug command。

`bottom_pane/slash_commands.rs` 额外负责：

- 根据 feature flags 过滤命令。
- 将动态 service-tier commands 插入 `/model` 附近。
- 支持 exact match、alias match、prefix/fuzzy match。
- 支持 `/goooooal` 这类趣味 alias 映射到 `/goal`。

### 3.2 完整命令表

| 命令 | alias / 说明 | 用途 | 主要模块 |
|---|---|---|---|
| `/model` | - | 选择模型 | `chatwidget/slash_dispatch.rs` |
| `/ide` | 支持 inline args | 包含或切换 IDE 上下文 | `chatwidget/slash_dispatch.rs` |
| `/permissions` | - | 打开权限配置 | `chatwidget/slash_dispatch.rs` |
| `/keymap` | 支持 inline args | 打开快捷键设置/调试 | `chatwidget/slash_dispatch.rs`, `keymap_setup.rs` |
| `/vim` | - | 开关 Vim 模式 | `chatwidget/slash_dispatch.rs`, `keymap.rs` |
| `/setup-default-sandbox` | enum 名 `ElevateSandbox` | Windows elevated sandbox 初始化 | `slash_command.rs`, `chatwidget/slash_dispatch.rs` |
| `/sandbox-add-read-dir` | enum 名 `SandboxReadRoot`，支持 inline args | Windows sandbox 添加只读根目录 | `slash_command.rs`, `chatwidget/slash_dispatch.rs` |
| `/experimental` | - | 打开实验特性开关 | `chatwidget/slash_dispatch.rs` |
| `/approve` | enum 名 `AutoReview` | 自动复核/批准被拒请求 | `chatwidget/slash_dispatch.rs` |
| `/memories` | - | 查看/管理记忆 | `chatwidget/slash_dispatch.rs` |
| `/skills` | `Tab` 补全时可直接打开 | 打开 skills 菜单 | `chatwidget/slash_dispatch.rs`, `chatwidget/skills.rs` |
| `/import` | - | 导入 Claude Code setup/project/recent chats | `external_agent_config_migration_flow.rs` |
| `/hooks` | - | 展示 hooks 相关输出/配置 | `chatwidget/slash_dispatch.rs` |
| `/review` | 支持 inline args | 代码审查流程 | `chatwidget/slash_dispatch.rs` |
| `/rename` | 支持 inline args | 重命名当前 thread | `chatwidget/slash_dispatch.rs` |
| `/new` | - | 新建 chat/session | `chatwidget/slash_dispatch.rs` |
| `/archive` | - | 归档当前 session 并退出 | `chatwidget/slash_dispatch.rs` |
| `/delete` | - | 删除当前 session 并退出 | `chatwidget/slash_dispatch.rs` |
| `/resume` | 支持 inline args | 打开 resume picker 或按 id/name 恢复 | `resume_picker.rs`, `chatwidget/slash_dispatch.rs` |
| `/fork` | - | fork 当前 session | `chatwidget/slash_dispatch.rs` |
| `/app` | macOS/Windows 可见 | 在 Codex Desktop 继续当前 thread | `chatwidget/slash_dispatch.rs`, `bottom_pane/app_link_view.rs` |
| `/init` | - | 使用内置 init prompt 创建 AGENTS.md | `chatwidget/slash_dispatch.rs` |
| `/compact` | - | 压缩/总结上下文 | `chatwidget/slash_dispatch.rs` |
| `/plan` | 支持 inline args | 进入 plan 模式或提交计划提示 | `chatwidget/slash_dispatch.rs` |
| `/goal` | 支持 inline args，另有 `/goooooal` 变体 | 打开/设置/暂停/恢复/清除 goal | `chatwidget/slash_dispatch.rs` |
| `/agent` | - | 打开 active agent picker | `chatwidget/slash_dispatch.rs` |
| `/subagents` | enum 名 `MultiAgents` | 多 agent / subagent 入口 | `chatwidget/slash_dispatch.rs` |
| `/side` | 支持 inline args | 开启临时 side conversation | `chatwidget/slash_dispatch.rs` |
| `/btw` | `/side` alias 风格 | 开启临时 side conversation | `chatwidget/slash_dispatch.rs` |
| `/copy` | Android 隐藏 | 复制最后一条 agent 回复 | `chatwidget/slash_dispatch.rs` |
| `/raw` | 支持 `on/off` inline args | 切换 raw scrollback 输出 | `chatwidget/slash_dispatch.rs` |
| `/diff` | - | 展示 git diff 和 untracked files | `chatwidget/slash_dispatch.rs` |
| `/mention` | - | 插入 `@`，触发 mention/file picker | `chatwidget/slash_dispatch.rs` |
| `/status` | - | 输出当前配置、模型、token、状态 | `chatwidget/slash_dispatch.rs`, `status/` |
| `/usage` | 支持 `daily/weekly/cumulative` | 打开账户用量视图 | `chatwidget/slash_dispatch.rs` |
| `/debug_config` | - | 输出 config layer 调试信息 | `chatwidget/slash_dispatch.rs` |
| `/title` | - | 设置 terminal title | `bottom_pane/title_setup.rs` |
| `/statusline` | - | 设置 status line | `bottom_pane/status_line_setup.rs` |
| `/theme` | - | 打开语法高亮 theme picker | `theme_picker.rs` |
| `/pets` | alias `/pet`，支持 inline args | terminal pet picker / 禁用 pet | `pets/picker.rs`, `pets/preview.rs` |
| `/mcp` | 支持 `verbose` inline arg | 展示 MCP server/tool inventory | `chatwidget/slash_dispatch.rs` |
| `/apps` | popup 中隐藏但可执行 | apps/connectors 管理 | `chatwidget/connectors.rs` |
| `/plugins` | - | plugin 管理 popup | `chatwidget/plugins*.rs` |
| `/logout` | - | 登出 | `chatwidget/slash_dispatch.rs` |
| `/quit` | popup 中作为 `/exit` alias 处理 | 退出 TUI | `chatwidget/slash_dispatch.rs` |
| `/exit` | - | 退出 TUI | `chatwidget/slash_dispatch.rs` |
| `/feedback` | - | 发送反馈/logs | `chatwidget/slash_dispatch.rs` |
| `/rollout` | debug build 可见 | 输出 rollout 文件路径 | `chatwidget/slash_dispatch.rs` |
| `/ps` | - | 列出后台终端进程 | `chatwidget/slash_dispatch.rs` |
| `/stop` | alias `/clean` | 停止后台终端进程 | `chatwidget/slash_dispatch.rs` |
| `/clear` | - | 清空 terminal UI 并开始新 chat UI 状态 | `chatwidget/slash_dispatch.rs` |
| `/personality` | feature gated | 打开 personality picker | `chatwidget/slash_dispatch.rs` |
| `/test-approval` | debug build 可见 | 触发测试 approval request | `chatwidget/slash_dispatch.rs` |
| `/debug-m-drop` | debug/internal | memory debug stub，不建议用户使用 | `slash_command.rs`, `chatwidget/slash_dispatch.rs` |
| `/debug-m-update` | debug/internal | memory debug stub，不建议用户使用 | `slash_command.rs`, `chatwidget/slash_dispatch.rs` |

### 3.3 inline args 支持

`slash_command.rs` 中声明支持 inline args 的命令：

- `/review <instructions>`
- `/rename <name>`
- `/plan <prompt>`
- `/goal clear|edit|pause|resume|<objective>`
- `/ide ...`
- `/keymap [debug]`
- `/mcp [verbose]`
- `/raw on|off`
- `/usage daily|weekly|cumulative`
- `/pets disable|hidden|off|none|<id>`
- `/side <message>`
- `/btw <message>`
- `/resume <id-or-name>`
- `/sandbox-add-read-dir <path>`

具体 inline dispatch 在：

- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`

### 3.4 运行期可用性

Codex 将命令按上下文过滤：

- side conversation 中只允许少量命令，例如 `/copy`、`/raw`、`/diff`、`/mention`、`/status`、`/usage`、`/ide`。
- task running 时会禁用会改变 session/context 的命令，例如 `/new`、`/archive`、`/delete`、`/fork`、`/init`、`/compact`、`/keymap`、`/vim`、`/experimental`、`/memories`、`/import`、`/review`、`/plan`、`/clear`、`/logout` 等。
- 部分命令受平台限制，例如 Windows sandbox 相关命令、macOS/Windows Desktop app command。
- debug-only 命令在 release/popup 中隐藏。

## 4. 快捷键和非 slash 交互

权威 keymap 定义：

- `D:\work\ai\codex\codex-rs\tui\src\keymap.rs`
- `D:\work\ai\codex\codex-rs\tui\src\key_hint.rs`

### 4.1 keymap 域

`RuntimeKeymap` 分域：

- `app`
- `chat`
- `composer`
- `editor`
- `vim_normal`
- `vim_operator`
- `vim_text_object`
- `pager`
- `list`
- `approval`

这种分域让快捷键提示、配置、冲突处理、事件匹配共享同一套结构。

### 4.2 App 级快捷键

主要默认绑定：

- `Ctrl-T`：打开 transcript overlay。
- `Ctrl-G`：打开外部编辑器。
- `Ctrl-O`：复制最后一条 agent 回复。
- `Ctrl-L`：清空 terminal UI。
- `Alt-R`：切换 raw output。
- `Alt-Left` / `Alt-Right`：在 agent threads 间导航，要求 composer 为空。
- `Esc`：中断、backtrack、取消 primed backtrack 等。
- `Enter`：确认 primed backtrack。

路由模块：

- `D:\work\ai\codex\codex-rs\tui\src\app\input.rs`

### 4.3 Chat/composer 快捷键

主要默认绑定：

- `Ctrl-C`：中断当前 turn；在特定状态下二次退出。
- `Ctrl-D`：空输入且无 popup 时退出。
- `Ctrl-V` / `Alt-V`：从剪贴板粘贴图片并作为附件。
- `BackTab`：切换 collaboration mode。
- `Alt-,` 或 `Shift-Down`：降低 reasoning effort。
- `Alt-.` 或 `Shift-Up`：提高 reasoning effort。
- `Alt-Up` 或 `Shift-Left`：编辑 queued message。
- `Enter`：提交。
- `Tab`：queue 当前输入。
- `?` / `Shift-?`：打开 shortcut overlay。
- `Ctrl-R`：history search previous。
- `Ctrl-S`：history search next。

路由模块：

- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\interaction.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer.rs`

### 4.4 Editor 和 Vim 模式

Editor 默认支持常见文本编辑键：

- 光标移动：方向键、`Ctrl-B/F/P/N`、Home/End、`Ctrl-A/E`。
- word movement：`Alt-B/F`、Ctrl/Alt 方向键。
- 删除：Backspace、Delete、`Ctrl-H/D/W/U/K`。
- yank：`Ctrl-Y`。
- newline：Enter、Shift-Enter、Alt-Enter、`Ctrl-J/M`。

Vim normal mode 支持：

- 进入 insert：`i/a/A/I/o/O`。
- 移动：`h/j/k/l`、方向键、`w/b/e`、`0/$`。
- 编辑：`x/s/D/C/Y/p`。
- operator：`d/y/c`。
- text object 和 operator-pending。
- `Esc` 取消。
- normal mode 下 `/` 进入 slash command popup，`!` 进入 shell/bash mode。

### 4.5 List、pager、approval 快捷键

List/picker：

- `Up`、`Ctrl-P`、`Ctrl-K`、`k`：上移。
- `Down`、`Ctrl-N`、`Ctrl-J`、`j`：下移。
- `Left` / `Ctrl-H`、`Right` / `Ctrl-L`：横向或层级移动。
- `PageUp` / `Ctrl-B`、`PageDown` / `Ctrl-F`。
- `Home` / `End`。
- `Enter` 接受。
- `Esc` 取消。

Pager：

- `Up/k`、`Down/j`。
- `PageUp`、`Shift-Space`、`Ctrl-B`。
- `PageDown`、`Space`、`Ctrl-F`。
- `Ctrl-U/D`。
- `Home/End`。
- `q`、`Ctrl-C` 关闭。
- `Ctrl-T` 关闭 transcript overlay。

Approval：

- `y`：批准。
- `a`：本 session 批准。
- `p`：按 prefix 批准。
- `d`：拒绝。
- `n` / `Esc`：decline。
- `c`：cancel。
- `Ctrl-A` / `Ctrl-Shift-A`：fullscreen approval。
- `o`：打开 thread。

## 5. Overlay、picker 和 control surfaces

### 5.1 Composer popup

模块：

- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer\popup_state.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\command_popup.rs`

能力：

- command popup。
- file search popup。
- skill popup。
- mention v2 popup。
- 同一时刻只允许一个 active popup。
- `Esc` dismiss，`Tab` 补全/选择，`Enter` 执行/选择，方向键移动。

### 5.2 Bottom pane control surfaces

模块：

- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\bottom_pane_view.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\approval_overlay.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\request_user_input\`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\mcp_server_elicitation*`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\title_setup.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\status_line_setup.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\app_link_view.rs`

能力：

- command approval。
- permissions approval。
- apply patch approval。
- MCP elicitation form。
- model-visible `request_user_input` UI。
- terminal title/status line setup。
- Desktop app link / handoff view。

### 5.3 Picker surfaces

常见 picker：

- resume/fork picker：`D:\work\ai\codex\codex-rs\tui\src\resume_picker.rs`
- cwd/fork prompt modal：`D:\work\ai\codex\codex-rs\tui\src\cwd_prompt.rs`
- theme picker：`D:\work\ai\codex\codex-rs\tui\src\theme_picker.rs`
- pets picker：`D:\work\ai\codex\codex-rs\tui\src\pets\picker.rs`
- keymap setup：`D:\work\ai\codex\codex-rs\tui\src\keymap_setup.rs`
- review branch/commit picker：由 `AppEvent::OpenReviewBranchPicker` / `OpenReviewCommitPicker` 触发。
- agent picker：由 `AppEvent::OpenAgentPicker` 触发。
- skills manager：`D:\work\ai\codex\codex-rs\tui\src\chatwidget\skills.rs`
- apps/connectors：`D:\work\ai\codex\codex-rs\tui\src\chatwidget\connectors.rs`
- plugins popup：`D:\work\ai\codex\codex-rs\tui\src\chatwidget\plugins*.rs`

### 5.4 Full-screen overlay

模块：

- `D:\work\ai\codex\codex-rs\tui\src\pager_overlay.rs`

能力：

- transcript overlay。
- static renderable overlay。
- scrollable pager。
- overlay key hints。
- alternate screen 渲染。

## 6. 会话、模型、权限、MCP、插件、子 agent 能力

### 6.1 会话能力

相关命令：

- `/new`
- `/resume`
- `/fork`
- `/archive`
- `/delete`
- `/rename`
- `/clear`
- `/compact`
- `/app`

关键模块：

- `D:\work\ai\codex\codex-rs\tui\src\resume_picker.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`
- `D:\work\ai\codex\codex-rs\tui\src\app_event.rs`
- `D:\work\ai\codex\codex-rs\tui\src\lib.rs`

设计特点：

- 顶层 `codex resume` / `codex fork` 通过内部 TUI CLI 字段进入 picker 或直接恢复指定 session。
- TUI 内 `/resume` 走 `OpenResumePicker`，也支持 inline id/name。
- `/archive` 和 `/delete` 走确认 selection view。
- `/app` 将当前 thread handoff 到 Codex Desktop。

### 6.2 模型和 reasoning 能力

相关入口：

- CLI `--model`
- CLI `--oss`
- CLI `--local-provider`
- `/model`
- reasoning effort 快捷键

关键模块：

- `D:\work\ai\codex\codex-rs\tui\src\lib.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`
- `D:\work\ai\codex\codex-rs\tui\src\app_event.rs`
- `D:\work\ai\codex\codex-rs\tui\src\keymap.rs`

设计特点：

- 启动时由 CLI/config 决定 provider、model、OSS provider。
- `/model` 打开 model picker。
- dynamic service-tier slash commands 可插入 `/model` 附近。
- reasoning effort 可用快捷键直接调节。

### 6.3 权限和 sandbox 能力

相关入口：

- CLI `--ask-for-approval`
- CLI `--sandbox`
- CLI `--dangerously-bypass-approvals-and-sandbox`
- CLI `--add-dir`
- `/permissions`
- `/approve`
- `/setup-default-sandbox`
- `/sandbox-add-read-dir`
- approval overlay 快捷键

关键模块：

- `D:\work\ai\codex\codex-rs\tui\src\lib.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\approval_overlay.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`
- `D:\work\ai\codex\codex-rs\tui\src\app_event.rs`

设计特点：

- 启动参数先落到 config/permission model。
- 运行时权限变更走 popup 或 approval overlay。
- Approval overlay 支持 exec、permissions、apply patch、MCP elicitation 等请求类型。
- Windows sandbox 有专门命令和 degraded/elevated setup flow。

### 6.4 MCP、apps/connectors、plugins

相关命令：

- `/mcp`
- `/apps`
- `/plugins`

关键模块：

- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\connectors.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\mcp_server_elicitation*`
- `D:\work\ai\codex\codex-rs\codex-mcp\src\`

设计特点：

- `/mcp` 展示 MCP server/tool inventory，并支持 `verbose`。
- `/apps` 管理 connectors/apps，但在 empty command popup 中隐藏。
- `/plugins` 打开 plugin 管理 popup。
- MCP elicitation 使用 bottom pane form/approval surface，而不是普通 chat 文本。

### 6.5 Skills、import、hooks

相关命令：

- `/skills`
- `/import`
- `/hooks`

关键模块：

- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\skills.rs`
- `D:\work\ai\codex\codex-rs\tui\src\external_agent_config_migration_flow.rs`
- `D:\work\ai\codex\codex-rs\tui\src\external_agent_config_migration\`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`

设计特点：

- `/skills` 是一等 TUI surface，且在 command popup 中有特殊 Tab 行为。
- `/import` 将外部 agent 配置迁移作为独立 flow。
- `/hooks` 输出当前 hooks 状态/配置，和 hook trust CLI 参数互补。

### 6.6 Goal、plan、side conversation、子 agent

相关命令：

- `/plan`
- `/goal`
- `/side`
- `/btw`
- `/agent`
- `/subagents`

关键模块：

- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`
- `D:\work\ai\codex\codex-rs\tui\src\app_event.rs`
- `D:\work\ai\codex\codex-rs\core\src\tools\handlers\multi_agents.rs`
- `D:\work\ai\codex\codex-rs\core\src\tools\handlers\multi_agents_common.rs`

设计特点：

- `/plan` 和 `/goal` 是用户意图管理入口，不只是普通 prompt。
- `/goal` 支持 `clear/edit/pause/resume/<objective>` inline 操作。
- `/side` / `/btw` 创建临时 side conversation，且 side conversation 中 slash command 可用范围被收窄。
- `/agent` / `/subagents` 把多 agent 能力暴露为 TUI 控制面，核心执行仍在 core tool handlers。

## 7. 关键源码路径索引

### CLI / startup

- `D:\work\ai\codex\codex-rs\cli\src\main.rs`
- `D:\work\ai\codex\codex-rs\tui\src\main.rs`
- `D:\work\ai\codex\codex-rs\tui\src\cli.rs`
- `D:\work\ai\codex\codex-rs\tui\src\lib.rs`
- `D:\work\ai\codex\codex-rs\utils\cli\src\shared_options.rs`

### Slash command

- `D:\work\ai\codex\codex-rs\tui\src\slash_command.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\slash_commands.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\command_popup.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer\slash_input.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\slash_dispatch.rs`

### Input / keymap

- `D:\work\ai\codex\codex-rs\tui\src\keymap.rs`
- `D:\work\ai\codex\codex-rs\tui\src\key_hint.rs`
- `D:\work\ai\codex\codex-rs\tui\src\app\input.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\interaction.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\chat_composer\popup_state.rs`

### Overlay / picker / bottom pane

- `D:\work\ai\codex\codex-rs\tui\src\app_event.rs`
- `D:\work\ai\codex\codex-rs\tui\src\pager_overlay.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\bottom_pane_view.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\approval_overlay.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\list_selection_view.rs`
- `D:\work\ai\codex\codex-rs\tui\src\bottom_pane\selection_list.rs`
- `D:\work\ai\codex\codex-rs\tui\src\resume_picker.rs`
- `D:\work\ai\codex\codex-rs\tui\src\cwd_prompt.rs`
- `D:\work\ai\codex\codex-rs\tui\src\theme_picker.rs`
- `D:\work\ai\codex\codex-rs\tui\src\keymap_setup.rs`
- `D:\work\ai\codex\codex-rs\tui\src\pets\picker.rs`
- `D:\work\ai\codex\codex-rs\tui\src\pets\preview.rs`

### Feature surfaces

- `D:\work\ai\codex\codex-rs\tui\src\status\`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\skills.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\connectors.rs`
- `D:\work\ai\codex\codex-rs\tui\src\chatwidget\plugins*.rs`
- `D:\work\ai\codex\codex-rs\tui\src\external_agent_config_migration_flow.rs`
- `D:\work\ai\codex\codex-rs\tui\src\external_agent_config_migration\`
- `D:\work\ai\codex\codex-rs\codex-mcp\src\`
- `D:\work\ai\codex\codex-rs\core\src\tools\handlers\multi_agents.rs`
- `D:\work\ai\codex\codex-rs\core\src\tools\handlers\multi_agents_common.rs`

## 8. 对 Agent-OS TUI 的借鉴点

### 8.1 建立单一 command registry

Codex 的 `SlashCommand` enum 是一个很好的模式：命令名、alias、说明、inline args、上下文可用性、平台可见性集中在一处。Agent-OS TUI 可以建立类似 registry，然后把执行分派到 kernel operation、thread runtime operation、CLI-local UI operation 三类后端。

建议：

- `agent-os-tui` 定义 command metadata。
- `agent-os-kernel` 继续拥有权威状态转换和权限决策。
- TUI command dispatch 只提交 kernel/runtime 请求，不复制 kernel authority。

### 8.2 输入解析和执行分离

Codex 把 slash input parsing 放在 composer 层，把 dispatch 放在 chatwidget 层。这避免了输入框直接知道所有业务逻辑。

Agent-OS 可采用：

- composer parser：识别 slash、inline args、queued command、shell command、mention。
- command dispatcher：做上下文检查和权限检查。
- kernel/runtime bridge：执行实际操作。

### 8.3 使用统一 BottomPaneView

Codex 的 approval、selection、request_user_input、MCP elicitation 都可以进入 bottom pane view。Agent-OS 可以把以下能力统一成 bottom pane control surfaces：

- permission approval。
- tool execution approval。
- MCP/tool/resource picker。
- profile/config picker。
- thread/session picker。
- replay/event inspector。
- model/context inspector。
- goal/plan editor。

### 8.4 keymap 分域并可配置

Codex 将 keymap 分为 app/chat/composer/editor/list/pager/approval。Agent-OS 也应避免在 UI 代码中散落硬编码快捷键。

建议：

- keymap metadata 是 UI hints 的唯一来源。
- list/pager/approval 使用统一导航语义。
- Windows/macOS/Linux 的差异在 key normalization 层解决。

### 8.5 overlay 用于长内容，popup 用于短选择

Codex 用 pager overlay 展示 transcript/static 长内容，用 bottom pane/popup 做短交互。Agent-OS 可以按内容尺度选择控制面：

- 长内容：event log、replay transcript、audit trace、model context projection。
- 短选择：model/profile/tool/session/permission。
- 表单：MCP elicitation、user input request、config import。

### 8.6 会话和子 agent 作为一等 TUI 对象

Codex 将 `/resume`、`/fork`、`/agent`、`/subagents`、`/side` 做成直接可见的 TUI surfaces。Agent-OS 如果要突出 kernel/thread 边界，也可以让这些能力一等化：

- thread/session picker。
- active agent selector。
- subagent tree/status view。
- side conversation 或 scoped scratch interaction。
- goal state indicator。

### 8.7 保持 forward-only 契约

对 Agent-OS 来说，最重要的是不要把 TUI 做成兼容层集合。可以借鉴 Codex 的交互组织，但按 Agent-OS 边界重新落位：

- `agent-os-sys`：共享 ABI/data types。
- `agent-os-kernel`：权限、状态转换、资源、tools、evidence、artifacts、replay、profile policy。
- `agent-os-thread`：model/client adapters 和 runtime loop。
- `agent-os-cli` / `agent-os-tui`：用户命令编排、展示、picker、overlay。

TUI 命令可以变化，public shape 可以为了更清晰的当前设计调整；关键是保持一个 canonical path，不为旧交互形态保留兼容 shim。

