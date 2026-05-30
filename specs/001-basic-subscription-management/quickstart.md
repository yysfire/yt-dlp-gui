# Quickstart: 基本订阅管理

**Feature**: 001-basic-subscription-management
**Date**: 2026-05-31

## 前置条件

1. 已安装 Rust 工具链 (`rustc`, `cargo`)
2. 已安装 Node.js 18+ 和 npm
3. 已安装 yt-dlp 命令行工具: `pip install yt-dlp` 或 `brew install yt-dlp`
4. 项目已 clone 并安装依赖: `npm install`

## 开发环境启动

```bash
# 终端 1: 启动 Tauri 开发模式 (自动启动 Vite + Rust)
npm run tauri dev

# 或分步启动:
# 终端 2: 仅启动 Vite 前端开发服务器
npm run dev
```

## 订阅管理功能验证

### 1. 添加订阅

1. 打开应用，点击左侧边栏的"添加订阅"按钮
2. 在弹出的对话框中输入 YouTube 或 Bilibili 频道 URL
3. 点击"添加"，等待频道信息解析
4. 验证：新订阅出现在左侧列表中，显示频道名称和封面图
5. 验证：重复添加同一频道时，系统提示"该频道已订阅"

### 2. 查看订阅列表

1. 添加 3-5 个不同类型的频道
2. 验证：列表显示每个订阅的名称、封面图、状态标识
3. 验证：空列表状态时显示引导提示

### 3. 删除订阅

1. 在订阅列表中，点击某个订阅旁的删除按钮
2. 在确认对话框中点击"确认"
3. 验证：该订阅从列表中移除
4. 验证：点击"取消"时订阅不变

### 4. 启用/禁用订阅

1. 点击订阅旁的暂停/启用切换按钮
2. 验证：禁用后显示暂停图标，启用后恢复正常
3. 验证：后台定时检查跳过已禁用的订阅

### 5. 手动检查更新

1. 点击某个订阅的"检查更新"按钮
2. 验证：状态栏显示检查进度
3. 点击"检查全部"按钮
4. 验证：所有已启用订阅依次检查

## 运行测试

```bash
# Rust 测试 (subscription 相关)
cargo test subscription
cargo test storage
cargo test ytdlp

# 全部测试
cargo test

# TypeScript 类型检查
npx tsc --noEmit
```

## 关键文件路径

| 文件 | 作用 |
|------|------|
| `src-tauri/src/commands/subscription.rs` | Tauri 命令处理 |
| `src-tauri/src/services/storage.rs` | JSON 文件存储 |
| `src-tauri/src/models/subscription.rs` | Subscription 数据模型 |
| `src-tauri/src/lib.rs` | 命令注册 (Tauri::generate_handler!) |
| `src/components/SubscriptionList.tsx` | 订阅列表 UI |
| `src/components/SubscriptionItem.tsx` | 单个订阅条目 |
| `src/components/AddSubscriptionDialog.tsx` | 添加订阅弹窗 |
| `src/hooks/useSubscriptions.ts` | 订阅状态管理 Hook |
| `src/lib/tauri.ts` | Tauri invoke 封装 |

## MVP 新增工作

| 任务 | 位置 | 描述 |
|------|------|------|
| 新增 `group_name` 字段 | `models/subscription.rs` | Subscription 结构体新增字段 |
| 新增 `last_checked_at` 字段 | `models/subscription.rs` | 记录最后检查时间 |
| 新增 `update_subscription_group` 命令 | `commands/subscription.rs` | 更新分组 |
| 注册新命令 | `src-tauri/src/lib.rs` | generate_handler 中添加 |
| 前端分组筛选 | `SubscriptionList.tsx` | 预定义分组下拉筛选 |
| 前端封装 | `src/lib/tauri.ts` | updateSubscriptionGroup invoke |
| 订阅条目显分组 | `SubscriptionItem.tsx` | 显示分组标签 |
