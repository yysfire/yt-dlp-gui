# 侧边栏布局优化：已下载导航

**日期**：2026-06-04  
**状态**：已批准

## 问题

当前左侧栏顶部为 `订阅 | 已下载` 水平标签，点击"已下载"后标签下方的侧边栏区域为空，大片空白不美观。

## 设计

### 布局

去掉顶部水平标签，改为垂直布局：

- **上方**：订阅列表（现有 SubscriptionList 组件，无改动）
- **下方**：固定在侧边栏底部的"已下载"全宽导航按钮，带图标和计数

```
┌──────────────────┐
│  搜索/添加订阅    │  ← SubscriptionList 操作栏（不变）
├──────────────────┤
│                  │
│  订阅频道 A      │
│  订阅频道 B      │  ← 订阅列表（垂直滚动）
│  订阅频道 C      │
│  ...             │
│                  │
├──────────────────┤
│  📂 已下载 (37)  │  ← 固定在底部，全宽按钮
└──────────────────┘
```

### 交互逻辑

1. 默认：选中第一个订阅（或无订阅），右侧显示详情面板，底部按钮未高亮
2. 点击底部按钮：右侧切换为 DownloadedList 视图，按钮高亮，取消所有订阅选中
3. 点击任意订阅项：右侧切换回详情面板，按钮取消高亮
4. `records-changed` 事件触发时自动刷新计数

### 改动范围

| 文件 | 改动 | 行数 |
|------|------|------|
| `src/components/AppShell.tsx` | 移除顶部标签栏；侧边栏底部新增固定导航按钮；视图切换逻辑调整 | ~30 行增删 |
| `DownloadedList.tsx` | 无需改动 | 0 |
| `SubscriptionList.tsx` | 无需改动 | 0 |

"已下载"按钮实现：

```tsx
<button
  onClick={() => {
    setActiveView("downloads");
    onSelectSubscription(null);
  }}
  className={`flex items-center gap-2 w-full px-3 py-2 text-sm border-t border-gray-200 dark:border-gray-700
    ${activeView === "downloads"
      ? "text-primary-600 dark:text-primary-400 bg-primary-50 dark:bg-primary-900/20"
      : "text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800"}`}
>
  <FolderOpenIcon fontSize="small" />
  <span className="flex-1 text-left">已下载</span>
  <span className="text-xs text-gray-400">
    {records.filter(r => r.status !== "deleted").length}
  </span>
</button>
```

### 自检

- **占位符**：无
- **一致性**：布局与现有侧边栏导航模式一致（底部固定项）
- **范围**：单组件改动，不涉及后端或新模块
- **模糊性**：无
