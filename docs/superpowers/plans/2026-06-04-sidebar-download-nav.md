# 侧边栏布局优化：已下载导航 实现计划

> **面向 AI 代理的工作者：** 使用 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将侧边栏顶部的 `订阅 | 已下载` 水平标签改为底部固定"已下载"导航按钮

**架构：** 仅改 `AppShell.tsx`（移除顶部标签栏，底部新增按钮 + 图标 + 计数；订阅列表始终显示）

**技术栈：** React 18 + TypeScript + MUI 5 + Tailwind CSS 3

---

### 任务 1：修改 AppShell 侧边栏布局

**文件：**
- 修改：`src/components/AppShell.tsx:1-240`

- [ ] **步骤 1：添加 FolderOpenIcon 导入**

在现有 `import` 区域追加 MUI 图标导入。

```typescript
// 在文件顶部 import 区域追加：
import { FolderOpen as FolderOpenIcon } from "@mui/icons-material";
```

- [ ] **步骤 2：添加下载计数 useMemo**

在 `selectedSub` / `filteredRecords` 行之后添加：

```typescript
  const downloadCount = records.filter((r) => r.status !== "deleted").length;
```

- [ ] **步骤 3：替换侧边栏 JSX**

定位到 `{/* Sidebar */}` 块（行 135-189），用新布局替换：

```tsx
        {/* Sidebar */}
        <div
          className={`flex-shrink-0 flex flex-col border-r border-gray-200 dark:border-gray-700 transition-all duration-200 ${
            sidebarCollapsed ? "w-0 overflow-hidden border-none" : "w-[280px]"
          }`}
        >
          {/* Subscription list — always visible */}
          <div className="flex-1 min-h-0">
            <SubscriptionList
              subscriptions={subscriptions}
              loading={subscriptionsLoading}
              error={subscriptionsError}
              selectedId={selectedId}
              onSelect={(id) => {
                setActiveView("detail");
                onSelectSubscription(id);
              }}
              onDelete={onDeleteSubscription}
              onTogglePause={onTogglePause}
              onCheckSubscription={onCheckSubscription}
              onManualCheckAll={onManualCheckAll}
              onRefresh={onRefreshSubscriptions}
              onOpenExport={() => setExportDialogOpen(true)}
              onOpenImport={() => setImportDialogOpen(true)}
              onUpdateGroup={onUpdateGroup}
            />
          </div>

          {/* Bottom: "已下载" navigation button */}
          {!sidebarCollapsed && (
            <button
              onClick={() => {
                setActiveView("downloads");
                onSelectSubscription(null);
              }}
              className={`flex items-center gap-2 w-full px-3 py-2.5 text-sm font-medium border-t border-gray-200 dark:border-gray-700 transition-colors
                ${activeView === "downloads"
                  ? "text-primary-600 dark:text-primary-400 bg-primary-50 dark:bg-primary-900/20"
                  : "text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800"}`}
            >
              <FolderOpenIcon fontSize="small" />
              <span className="flex-1 text-left">已下载</span>
              <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
                {downloadCount}
              </span>
            </button>
          )}
        </div>
```

关键变化：
- 删除了 `{/* Navigation tabs */}` 块（行 141-171）
- 侧边栏增加 `flex flex-col`，订阅列表用 `flex-1 min-h-0` 占满空间
- 订阅列表始终渲染，去掉 `activeView === "detail"` 条件
- `onSelect` 回调中同步设置 `setActiveView("detail")`
- 底部按钮在 `sidebarCollapsed` 时也隐藏

- [ ] **步骤 4：npm run build 验证**

```bash
cd /home/yys/projects/yt-dlp-gui && npm run build
```

预期：构建成功，无 TypeScript 错误。

- [ ] **步骤 5：Commit**

```bash
cd /home/yys/projects/yt-dlp-gui
git add src/components/AppShell.tsx
git commit -m "refactor(sidebar): 将已下载导航从顶部标签改为底部按钮

- 移除顶部订阅/已下载水平标签
- 订阅列表始终在侧边栏可见
- 底部新增带图标+计数的已下载导航按钮
- 点击任意订阅自动回到详情视图

Co-Authored-By: CodeBuddy CLI
Co-Authored-By: DeepSeek-V4-Pro <noreply@deepseek.com>"
```

---

### 自检

1. **规格覆盖度**：所有设计文档要求均已覆盖 —— 移除标签 ✓、底部按钮 ✓、图标 ✓、计数 ✓、点击联动 ✓
2. **占位符扫描**：无
3. **类型一致性**：`FolderOpenIcon` 来自 `@mui/icons-material`，`downloadCount` 类型为 `number`，均与使用处一致
