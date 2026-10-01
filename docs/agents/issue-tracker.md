# Issue 跟踪：GitHub

本仓库的 issue 与 spec 以 GitHub issue 形式存在。所有操作使用 `gh` CLI。

## 约定

- **创建 issue**：`gh issue create --title "..." --body "..."`。多行 body 用 heredoc。
- **读取 issue**：`gh issue view <number> --comments`，用 `jq` 过滤评论，同时拉取 labels。
- **列出 issue**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，配合相应的 `--label` 与 `--state` 过滤。
- **评论 issue**：`gh issue comment <number> --body "..."`
- **添加 / 移除标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭**：`gh issue close <number> --comment "..."`

仓库由 `git remote -v` 推断；在克隆目录内运行 `gh` 时会自动识别。

## 把 Pull Request 作为 triage 入口

**PR 作为请求入口：否。** _（若本仓库把外部 PR 当作功能请求处理，改为 `yes`；`/triage` 会读取此开关。）_

设为 `yes` 时，PR 与 issue 走同一套标签与状态，命令改用 `gh pr` 对应项：

- **读取 PR**：`gh pr view <number> --comments`，diff 用 `gh pr diff <number>`。
- **列出待 triage 的外部 PR**：`gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments`，只保留 `authorAssociation` 为 `CONTRIBUTOR`、`FIRST_TIME_CONTRIBUTOR`、`NONE` 的项（剔除 `OWNER`/`MEMBER`/`COLLABORATOR`）。
- **评论 / 打标签 / 关闭**：`gh pr comment`、`gh pr edit --add-label`/`--remove-label`、`gh pr close`。

GitHub 的 issue 与 PR 共用一套编号空间，因此裸写的 `#42` 可能是两者之一：先用 `gh pr view 42` 解析，失败则回退 `gh issue view 42`。

## 当某项技能说「publish to the issue tracker」

创建一个 GitHub issue。

## 当某项技能说「fetch the relevant ticket」

运行 `gh issue view <number> --comments`。

## Wayfinding 操作

由 `/wayfinder` 使用。**map** 是单个 issue，**子** issue 作为 ticket。

- **Map**：一个带 `wayfinder:map` 标签的 issue，承载 Notes / Decisions-so-far / Fog 正文。`gh issue create --label wayfinder:map`。
- **子 ticket**：作为 map 的 GitHub sub-issue 关联的 issue（对 sub-issues endpoint 调用 `gh api`）。若未启用 sub-issue，则把子项加入 map 正文的任务列表，并在子 issue 正文顶部写 `Part of #<map>`。标签：`wayfinder:<type>`（`research`/`prototype`/`grilling`/`task`）。认领后，把 ticket 指派给推进的开发者。
- **阻塞**：GitHub 原生 issue 依赖，是规范且在 UI 中可见的表达。用 `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>` 添加边，其中 `<blocker-db-id>` 是阻塞项的**数字 database id**（`gh api repos/<owner>/<repo>/issues/<n> --jq .id`，**不是** `#number` 也不是 `node_id`）。GitHub 以 `issue_dependencies_summary.blocked_by` 报告（仅未关闭的阻塞项，是实时的门槛）。若依赖功能不可用，回退到子 issue 正文顶部的 `Blocked by: #<n>, #<n>` 行。当所有阻塞项都关闭时，该 ticket 解除阻塞。
- **Frontier 查询**：列出 map 的未关闭子项（`gh issue list --state open`，限定在 map 的 sub-issues / 任务列表内），剔除任何有未关闭阻塞项（`issue_dependencies_summary.blocked_by > 0`，或 `Blocked by` 行中仍有未关闭 issue）或有 assignee 的项；按 map 顺序取第一个。
- **认领**：`gh issue edit <n> --add-assignee @me`，是本会话的第一次写操作。
- **解决**：`gh issue comment <n> --body "<answer>"`，然后 `gh issue close <n>`，再把上下文指针（gist + 链接）追加到 map 的 Decisions-so-far。
