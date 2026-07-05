# OpenCode TUI/CLI 用户命令能力调研

调研对象：`D:\work\ai\opencode`

调研日期：2026-07-05

调研范围：只读分析 OpenCode 启动后的终端、TUI、CLI 用户能力。重点关注用户可见命令、slash commands、快捷键、面板/模式、会话/模型/工具/权限/配置入口。本文不覆盖底层实现细节、协议实现或渲染细节。

## 1. CLI 启动命令

OpenCode 是 Bun/TypeScript monorepo。CLI 入口由 `yargs` 注册命令，TUI 使用 OpenTUI + Solid。

关键入口：

- `D:\work\ai\opencode\packages\opencode\bin\opencode`：Node 启动 shim，选择平台原生二进制，支持 `OPENCODE_BIN_PATH`。
- `D:\work\ai\opencode\packages\opencode\src\index.ts`：CLI 总入口，注册所有顶层命令。
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\tui.ts`：默认 TUI 命令。
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\run.ts`：非交互/mini 运行命令。
- `D:\work\ai\opencode\packages\tui\package.json`：TUI 依赖，包含 `@opentui/core`、`@opentui/keymap`、`@opentui/solid`、`solid-js` 等。

主要启动形态：

| 命令 | 用途 |
| --- | --- |
| `opencode [project]` | 默认全屏 TUI。 |
| `opencode run [message..]` | 非交互执行提示，或进入 split-footer/mini 交互。 |
| `opencode attach <url>` | 连接正在运行的 OpenCode server。 |
| `opencode serve` | 启动 headless server。 |
| `opencode web` | 启动 server + Web UI。 |

默认 TUI 常用参数：

- `--model, -m`：指定模型。
- `--continue, -c`：继续最近 session。
- `--session, -s`：指定 session。
- `--fork`：fork 指定 session。
- `--prompt`：启动时带入 prompt。
- `--agent`：指定 agent。
- `--auto`：自动批准未显式 deny 的权限请求。
- `--mini`：使用 mini/split-footer 交互界面。

`opencode run [message..]` 常用参数：

- `--command`：以 command/slash command 形式执行输入。
- `--format json`：JSON 输出。
- `--interactive`：交互模式。
- `--file`：附加文件。
- `--session`、`--continue`、`--fork`：会话选择。
- `--model`、`--agent`：模型/agent 选择。
- `--attach`、`--port`、`--username`、`--password`：连接已有 server。

其他 CLI 命令：

| 命令 | 用途 |
| --- | --- |
| `opencode models [provider]` | 列出 provider/model，支持刷新和 verbose。 |
| `opencode providers` / `opencode auth` | provider 登录、登出、列表。 |
| `opencode mcp` | MCP add/list/auth/logout/debug。 |
| `opencode agent` | 创建或列出 agent。 |
| `opencode session` | session list/delete。 |
| `opencode stats` | 按天、工具、模型、项目统计。 |
| `opencode export` / `opencode import` | 导入导出 session JSON。 |
| `opencode github` | GitHub 集成命令。 |
| `opencode pr <number>` | 处理 Pull Request。 |
| `opencode plugin` / `opencode plug` | 插件安装/管理入口。 |
| `opencode db` | 数据库路径/查询调试。 |
| `opencode debug` | 调试命令族。 |
| `opencode upgrade` | 升级 OpenCode。 |
| `opencode uninstall` | 卸载 OpenCode。 |
| `opencode acp` | ACP 相关命令。 |

## 2. TUI 架构

TUI 的用户命令体系不是散落在输入框里，而是由 command registry/keymap/palette/slash autocomplete 组合出来。

核心结构：

- `D:\work\ai\opencode\packages\tui\src\app.tsx`：TUI app 根组件，注册全局 UI commands、slash commands、dialogs。
- `D:\work\ai\opencode\packages\tui\src\keymap.tsx`：把 keymap command 转换为 slash autocomplete 和 command palette 可见命令。
- `D:\work\ai\opencode\packages\tui\src\config\keybind.ts`：默认快捷键定义和 command id。
- `D:\work\ai\opencode\packages\tui\src\component\command-palette.tsx`：command palette 展示、搜索、执行。
- `D:\work\ai\opencode\packages\tui\src\component\prompt\index.tsx`：prompt 输入框、slash 解析、shell 模式、提交逻辑。
- `D:\work\ai\opencode\packages\tui\src\component\prompt\autocomplete.tsx`：slash、文件引用、server commands 自动补全。
- `D:\work\ai\opencode\packages\tui\src\routes\session\index.tsx`：session 页面命令、消息导航、分享、fork、compact 等。

TUI 页面/面板能力：

- Home：logo、初始 prompt、home footer/tips 插件位。
- Session：消息时间线、prompt、permission/question panel、sidebar、timeline、diff dialog。
- Sidebar：session 标题、share/workspace 状态、context/MCP/LSP/todo/files 等插件区块。
- Dialogs：model、provider、agent、MCP、theme、session、stash、skill、status、debug、export、move、workspace。
- Feature plugins：notifications、plugin manager、which-key、diff viewer、sidebar panels。

## 3. Slash commands 完整清单

以下清单基于源码中的 `slashName` / `slashAliases` 注册点，以及 server command 注入逻辑整理。

### 3.1 全局 App slash commands

来源：`D:\work\ai\opencode\packages\tui\src\app.tsx`

| Slash command | 别名 | 用途 |
| --- | --- | --- |
| `/sessions` | `/resume`, `/continue` | 打开 session 列表，切换/恢复会话。 |
| `/new` | `/clear` | 新建 session，回到新对话状态。 |
| `/workspaces` | - | 管理 workspace。 |
| `/models` | `/mo` | 打开模型选择器。 |
| `/agents` | - | 打开 agent 选择器。 |
| `/mcps` | - | 打开 MCP 管理/开关面板。 |
| `/variants` | - | 打开 model variant 选择器。 |
| `/connect` | - | 连接 provider。 |
| `/org` | `/orgs`, `/switch-org` | 切换 Console organization。 |
| `/status` | - | 查看当前状态。 |
| `/debug` | - | 查看 debug 信息。 |
| `/themes` | - | 打开主题选择器。 |
| `/help` | - | 打开帮助。 |
| `/exit` | `/quit`, `/q` | 退出 TUI。 |

### 3.2 Prompt slash commands

来源：`D:\work\ai\opencode\packages\tui\src\component\prompt\index.tsx`

| Slash command | 用途 |
| --- | --- |
| `/editor` | 用外部 `$EDITOR` 编辑当前 prompt。 |
| `/skills` | 打开 skill selector，并插入选择的 skill command。 |
| `/warp` | 设置 workspace。 |
| `/move` | 将当前 session 移动到另一个项目目录。 |

### 3.3 Session slash commands

来源：`D:\work\ai\opencode\packages\tui\src\routes\session\index.tsx`

| Slash command | 别名 | 用途 |
| --- | --- | --- |
| `/share` | - | 分享 session 或复制分享链接。 |
| `/rename` | - | 重命名 session。 |
| `/timeline` | - | 打开消息 timeline，跳转到指定消息。 |
| `/fork` | - | 从 timeline 中的某条消息 fork session。 |
| `/compact` | `/summarize` | 压缩/总结上下文。 |
| `/unshare` | - | 取消 session 分享。 |
| `/undo` | - | 撤销上一条用户消息，并恢复 prompt。 |
| `/redo` | - | 恢复被撤销的消息。 |
| `/timestamps` | `/toggle-timestamps` | 显示/隐藏消息时间戳。 |
| `/thinking` | `/toggle-thinking` | 显示/隐藏 reasoning/thinking blocks。 |
| `/copy` | - | 复制 session transcript。 |
| `/export` | - | 导出 session transcript。 |

### 3.4 Feature plugin slash commands

来源：`D:\work\ai\opencode\packages\tui\src\feature-plugins\system\diff-viewer.tsx`

| Slash command | 用途 |
| --- | --- |
| `/diff` | 打开 diff viewer。 |

### 3.5 Server/project slash commands

来源：

- `D:\work\ai\opencode\packages\opencode\src\command\index.ts`
- `D:\work\ai\opencode\packages\opencode\src\session\prompt.ts`
- `D:\work\ai\opencode\packages\tui\src\component\prompt\autocomplete.tsx`

| Command 来源 | 用户可见能力 |
| --- | --- |
| 内置 `/init` | 引导生成/更新 `AGENTS.md`。 |
| 内置 `/review` | 审查当前改动、commit、branch 或 PR。 |
| 配置目录 `{command,commands}/**/*.md` | 项目/用户自定义 markdown commands。 |
| MCP prompts | MCP server 暴露的 prompts 作为 commands。 |
| Skills | 通过 skill 系统暴露，TUI 主要经 `/skills` 选择。 |

执行方式：

- 当输入以 `/` 开头且命中 server command 时，TUI 调用 `session.command`。
- command template 支持参数替换、`$ARGUMENTS`、shell output blocks、`@` 引用解析。
- command 可根据配置作为普通 prompt 或 subtask 执行。

备注：OpenCode 文档中仍可见 `/details` 描述，但本次源码搜索未看到对应 `slashName: "details"` 注册；当前源码里更像是通过 tool details/keybind/palette 行为暴露，属于文档与源码可能存在漂移的点。

### 3.6 Mini/split-footer slash commands

来源：

- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\run\footer.prompt.tsx`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\run\footer.command.tsx`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\run\prompt.shared.ts`

Mini 模式内置 slash：

| Slash command | 用途 |
| --- | --- |
| `/editor` | 打开外部编辑器。 |
| `/new` | 新建 session。 |
| `/exit` | 退出。 |
| `/quit` | 退出别名。 |
| `:q` | 退出别名。 |
| `/skills` | 当存在 skill commands 且无普通 `skills` command 时显示。 |

Mini 模式也会合并 server commands，包括项目 commands 和 MCP commands。

## 4. 快捷键/非 slash 交互

默认 keymap 来源：`D:\work\ai\opencode\packages\tui\src\config\keybind.ts`

### 4.1 全局与 leader key

| 快捷键 | 用途 |
| --- | --- |
| `ctrl+p` | 打开 command palette。 |
| `ctrl+x` | leader key。 |
| `ctrl+x q` | 退出。 |
| `ctrl+x n` | 新建 session。 |
| `ctrl+x l` | session 列表。 |
| `ctrl+x c` | compact。 |
| `ctrl+x x` | export。 |
| `ctrl+x m` | models。 |
| `ctrl+x a` | agents。 |
| `ctrl+x b` | sidebar。 |
| `ctrl+x s` | status。 |
| `ctrl+x t` | themes。 |
| `ctrl+x g` | timeline。 |
| `ctrl+x y` | 复制消息。 |
| `ctrl+x u` | undo。 |
| `ctrl+x r` | redo。 |
| `ctrl+x 1..9` | session quick slots。 |

### 4.2 模型、agent、variant

| 快捷键 | 用途 |
| --- | --- |
| `tab` | 切换 agent。 |
| `shift+tab` | 反向切换 agent。 |
| `ctrl+t` | 切换 model variant。 |
| `f2` | 切换最近模型。 |
| `shift+f2` | 反向切换最近模型。 |

### 4.3 Prompt 输入

| 快捷键/输入 | 用途 |
| --- | --- |
| `enter` | 提交 prompt。 |
| `shift+enter` / `ctrl+enter` / `alt+enter` / `ctrl+j` | 插入换行。 |
| `ctrl+c` | 清空输入，或中断运行中的 session。 |
| `ctrl+v` | 粘贴。 |
| `/` | 打开 slash autocomplete。 |
| `@` | 打开文件/引用 autocomplete。 |
| `!` | 进入 shell command 模式。 |

Prompt 还支持常见文本编辑快捷键，包括左右移动、行首行尾、按词移动、删除词、删除行、全选等。

### 4.4 消息滚动与导航

| 快捷键 | 用途 |
| --- | --- |
| `pageup` / `pagedown` | 翻页。 |
| `home` / `end` | 跳到首/尾。 |
| `ctrl+alt+b` / `ctrl+alt+f` | 上/下翻页类导航。 |
| `ctrl+alt+y` / `ctrl+alt+e` | 按行滚动。 |
| `ctrl+g` | 第一条消息。 |
| `ctrl+alt+g` | 最后一条消息。 |

### 4.5 Dialog、permission、question、diff

Dialog 通用交互：

- `up/down` 或 `ctrl+p/ctrl+n`：上下移动。
- `pageup/pagedown`：翻页。
- `home/end`：首尾。
- `return`：选择。
- `escape`：关闭。

Permission prompt：

- 左右选择 permission option。
- `return` 确认。
- `escape` 拒绝。
- `ctrl+f` 全屏。
- 可选项通常是 `Allow once`、`Allow always`、`Reject`。

Question prompt：

- `1-9` 快速选择。
- `up/down`、`j/k`、`tab` 切换。
- `return` 选择/提交。
- `escape` 拒绝。
- 支持 custom answer 编辑。

Diff viewer：

- `escape` / `q`：关闭。
- `enter` / `space`：切换/展开。
- `left/right`：折叠/展开。
- `tab`：切换 focus。
- `[` / `]`：hunk 导航。
- `n` / `p`：文件导航。
- `b`：切换 tree。
- `s`：single patch。
- `d`：source。
- `v`：split/unified。
- `?`：帮助。

## 5. 会话、模型、工具、权限、配置能力

### 5.1 会话能力

CLI 层：

- `--continue`：继续最近 session。
- `--session`：指定 session。
- `--fork`：fork session。
- `opencode session list/delete`：列出/删除 session。
- `opencode export/import`：导入导出 session JSON。

TUI 层：

- 新建 session。
- 切换/恢复 session。
- 重命名 session。
- 分享/取消分享 session。
- timeline 跳转。
- 从指定消息 fork。
- compact/summarize。
- undo/redo。
- 复制/导出 transcript。
- 移动 session 到其他项目目录。
- 子 agent/父 session 导航。

关键路径：

- `D:\work\ai\opencode\packages\tui\src\routes\session\index.tsx`
- `D:\work\ai\opencode\packages\opencode\src\server\routes\instance\httpapi\groups\session.ts`

### 5.2 模型、provider、agent 能力

模型选择来源：

- CLI `--model`。
- 配置中的默认 model。
- 最近使用 model。
- provider 默认 model。

TUI 能力：

- Model dialog：按 provider 展示模型、收藏、最近模型、免费标记、不可用状态。
- Provider connect dialog：API key、OAuth、custom provider。
- Agent dialog：选择当前 agent。
- Variant selector：切换 thinking/variant 等模型变体。

内置 agents：

- `build`：默认 primary agent。
- `plan`：规划模式，权限更保守。
- `general`：通用 subagent。
- `explore`：探索型 subagent。
- `compaction`、`title`、`summary`：隐藏/内部用途 agent。

关键路径：

- `D:\work\ai\opencode\packages\tui\src\context\local.tsx`
- `D:\work\ai\opencode\packages\tui\src\component\dialog-model.tsx`
- `D:\work\ai\opencode\packages\tui\src\component\dialog-provider.tsx`
- `D:\work\ai\opencode\packages\tui\src\component\dialog-agent.tsx`
- `D:\work\ai\opencode\packages\opencode\src\agent\agent.ts`

### 5.3 工具能力

内置工具注册点：`D:\work\ai\opencode\packages\opencode\src\tool\registry.ts`

主要内置工具：

- question。
- shell/bash。
- read。
- glob。
- grep。
- edit。
- write。
- task。
- webfetch。
- todowrite。
- websearch。
- skill。
- apply_patch。
- code mode。
- 可选 LSP。
- 可选 plan。

MCP 工具：

- MCP tools 会作为普通 tool 注入。
- MCP resources 会暴露 `list_mcp_resources`、`list_mcp_resource_templates`、`read_mcp_resource`。
- MCP tool/resource 也经过权限系统。

关键路径：

- `D:\work\ai\opencode\packages\opencode\src\session\tools.ts`
- `D:\work\ai\opencode\packages\opencode\src\tool\shell.ts`

### 5.4 权限能力

权限核心：`D:\work\ai\opencode\packages\opencode\src\permission\index.ts`

权限决策：

- `allow`：允许。
- `ask`：询问用户。
- `deny`：拒绝。
- 默认不匹配时为 `ask`。

TUI permission prompt：

- `Allow once`：本次允许。
- `Allow always`：当前运行期/session 范围内持续允许。
- `Reject`：拒绝。

其他特性：

- `--auto` 会自动批准未显式 deny 的权限请求。
- agent 可以带默认权限策略。
- 用户配置可定义 permission rules。
- 对 shell、文件读取、外部目录、`.env` 等敏感行为有专门判断路径。

关键路径：

- `D:\work\ai\opencode\packages\tui\src\routes\session\permission.tsx`
- `D:\work\ai\opencode\packages\opencode\src\agent\agent.ts`

### 5.5 配置能力

主配置入口：`D:\work\ai\opencode\packages\opencode\src\config\config.ts`

主配置来源：

- 全局 `opencode.jsonc` / `opencode.json` / `config.json`。
- 项目目录向上查找的 `opencode.json(c)`。
- `.opencode/opencode.json(c)`。
- `OPENCODE_CONFIG`。
- `OPENCODE_CONFIG_CONTENT`。
- managed config。
- remote well-known/console config。

配置扩展：

- Commands：`{command,commands}/**/*.md`。
- Agents：`{agent,agents}/**/*.md`、`{mode,modes}/*.md`。
- Plugins：通过配置和插件目录加载。
- Tools/permissions：旧 tools boolean 会转换为当前 permission 表达。
- `OPENCODE_PERMISSION`：环境变量权限配置入口。

TUI 配置：

- TUI 配置与运行时配置分离。
- 支持全局、项目、`.opencode`、`OPENCODE_TUI_CONFIG`。
- 字段包括 `$schema`、`theme`、`keybinds`、`plugin`、`plugin_enabled`、`leader_timeout`、`attention`、`prompt`、`scroll_speed`、`scroll_acceleration`、`diff_style`、`mouse`。

关键路径：

- `D:\work\ai\opencode\packages\opencode\src\config\paths.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\command.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\agent.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\tui.ts`
- `D:\work\ai\opencode\packages\tui\src\config\index.tsx`

## 6. 关键源码路径

CLI 与启动：

- `D:\work\ai\opencode\packages\opencode\bin\opencode`
- `D:\work\ai\opencode\packages\opencode\src\index.ts`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\tui.ts`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\run.ts`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\attach.ts`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\serve.ts`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\providers.ts`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\mcp.ts`
- `D:\work\ai\opencode\packages\opencode\src\cli\cmd\agent.ts`

TUI 命令、快捷键、面板：

- `D:\work\ai\opencode\packages\tui\src\app.tsx`
- `D:\work\ai\opencode\packages\tui\src\keymap.tsx`
- `D:\work\ai\opencode\packages\tui\src\config\keybind.ts`
- `D:\work\ai\opencode\packages\tui\src\config\index.tsx`
- `D:\work\ai\opencode\packages\tui\src\component\command-palette.tsx`
- `D:\work\ai\opencode\packages\tui\src\component\prompt\index.tsx`
- `D:\work\ai\opencode\packages\tui\src\component\prompt\autocomplete.tsx`
- `D:\work\ai\opencode\packages\tui\src\routes\home.tsx`
- `D:\work\ai\opencode\packages\tui\src\routes\session\index.tsx`
- `D:\work\ai\opencode\packages\tui\src\routes\session\sidebar.tsx`
- `D:\work\ai\opencode\packages\tui\src\routes\session\permission.tsx`
- `D:\work\ai\opencode\packages\tui\src\routes\session\question.tsx`

Commands、tools、permissions、config：

- `D:\work\ai\opencode\packages\opencode\src\command\index.ts`
- `D:\work\ai\opencode\packages\opencode\src\session\prompt.ts`
- `D:\work\ai\opencode\packages\opencode\src\session\tools.ts`
- `D:\work\ai\opencode\packages\opencode\src\tool\registry.ts`
- `D:\work\ai\opencode\packages\opencode\src\tool\shell.ts`
- `D:\work\ai\opencode\packages\opencode\src\permission\index.ts`
- `D:\work\ai\opencode\packages\opencode\src\agent\agent.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\config.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\paths.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\command.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\agent.ts`
- `D:\work\ai\opencode\packages\opencode\src\config\tui.ts`

## 7. 对 Agent-OS TUI 的借鉴点

1. 建立统一 command registry。

   OpenCode 的 TUI action、slash command、command palette、keybind 基本围绕 command metadata 工作。Agent-OS TUI 可以采用类似结构：一个 command id 对应标题、描述、分类、是否可见、是否 suggested、slash 名称、默认快捷键和执行函数。

2. 区分 UI action command 与 model/runtime command。

   OpenCode 的 `/models`、`/themes`、`/sessions` 是 UI action；`/init`、`/review`、自定义 markdown command 是 server/runtime command。Agent-OS 也应避免把所有 slash 都塞进同一执行路径。

3. 用 palette 作为能力总入口。

   command palette 对新用户友好，也能承载隐藏但可搜索的高级能力，例如 debug、copy path、toggle、heap snapshot、terminal suspend。

4. 把模型、agent、MCP、权限、配置做成第一等面板。

   OpenCode 没有把这些能力藏在配置文件或命令参数里，而是提供专门 dialog。Agent-OS TUI 可以把运行时对象做成可浏览、可搜索、可切换的界面。

5. 权限 UX 分层清楚。

   配置层定义长期策略，运行时 permission prompt 提供 `once/always/reject`。Agent-OS 可以沿用这种模式，并明确展示本次授权的 tool、pattern、scope、风险说明。

6. Keybind 应绑定 command id，而不是组件行为。

   这样可以天然支持用户覆盖、which-key、帮助生成、冲突检查和平台差异。

7. Mini/非交互模式值得保留。

   OpenCode 的 `run`/mini 模式让同一套能力进入脚本、CI、远端 attach、轻量交互场景。Agent-OS 可考虑把 TUI 能力分成 full TUI、mini composer、non-interactive run 三层。

8. 配置与扩展入口应文件化、可发现。

   Markdown commands、Markdown agents、TUI 独立配置、MCP、plugins 都是用户可扩展入口。Agent-OS 可借鉴这种“源码友好”的扩展形态。

9. 文档最好由 registry 生成。

   本次调研发现 OpenCode 文档可能存在 `/details` 这类源码/文档漂移。Agent-OS 若采用统一 registry，可以自动生成 help、slash 列表、keybind 文档，减少漂移。

10. 面板/模式栈要显式。

    OpenCode 对 prompt、autocomplete、permission、question、diff、dialog 等不同状态有不同 key handling。Agent-OS TUI 应明确 mode stack 和输入优先级，避免快捷键在复杂状态下相互踩踏。
