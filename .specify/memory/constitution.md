<!--
  Sync Impact Report
  ==================
  Version change: 0.0.0 → 1.0.0 (初始制定)
  Modified principles: N/A (首次填充模板)
  Added sections:
    - Core Principles (5 条原则：TDD、可测试性、KISS、DRY、YAGNI)
    - Additional Constraints（技术栈约束）
    - Development Workflow（开发工作流）
    - Governance
  Removed sections: N/A
  Templates requiring updates:
    - .specify/templates/plan-template.md → ✅ 无需修改（Constitution Check 为占位符，运行时动态填充）
    - .specify/templates/spec-template.md → ✅ 无需修改（用户故事优先级结构已对齐）
    - .specify/templates/tasks-template.md → ✅ 无需修改（测试任务模板已覆盖 TDD 流程）
    - .specify/templates/checklist-template.md → ✅ 无需修改（通用模板）
  Follow-up TODOs: 无
-->

# yt-dlp-gui 项目章程

## 核心原则

### I. 测试驱动开发（TDD）— 不可协商

所有功能开发必须遵循 TDD 循环：**编写测试 → 测试失败 → 实现代码 → 测试通过 → 重构**。

- 新增任何功能前，MUST 先编写失败的测试用例。
- 测试 MUST 在实现前被用户审查确认（或由 AI 代理生成并展示给用户）。
- 仅在测试全部通过后才可提交代码。
- 前端组件 MUST 通过手动测试或 React Testing Library 验证关键交互路径。
- Rust 后端 MUST 在 `#[cfg(test)]` 模块中包含对新功能的单元测试。

### II. 代码可测试性优先

架构设计 MUST 考虑可测试性，避免不可测试的代码结构。

- Services 层 MUST 保持无状态，通过参数接收依赖，禁止全局可变状态。
- StorageService 和 YtDlpService 的接口 MUST 支持通过参数注入路径/配置，便于测试隔离。
- 前端 Hooks（useSubscriptions、useDownloadRecords）MUST 通过 `src/lib/tauri.ts` 封装层调用后端，禁止组件内直接 `invoke()`。
- 禁止硬编码文件路径、URL 或配置值 —— 所有可变值 MUST 通过参数或配置传入。

### III. KISS（保持简单）

优先选择最简单的可行方案，抵制不必要的复杂性。

- 数据结构 SHOULD 优先使用 JSON 文件存储，除非有明确证据显示需要数据库。
- 新功能 SHOULD 复用现有组件和 Service，而非创建新的抽象。
- 禁止为"未来可能的需求"预留接口或抽象 —— 遵循 YAGNI。

### IV. DRY（不要重复自己）

相同逻辑不应在多处出现。

- 重复三次以上的代码 MUST 提取为公共函数/组件。
- 前端 `src/lib/tauri.ts` 是后端调用的唯一入口，禁止在其他文件中直接调用 `invoke()`。
- Rust Commands 层的错误处理 MUST 统一使用 `AppError` 枚举，禁止各自定义错误类型。

### V. YAGNI（你不会需要它）

不实现当前需求不需要的功能。

- 每个 PR/提交 MUST 仅包含当前任务明确要求的功能。
- 禁止引入未在当前功能中使用的依赖包。
- 禁止编写"预留扩展点"、"未来接口"或"待启用"的注释代码块。
- 当新增抽象（如新的 trait、新的 Hook、新的 Service）时，MUST 能清晰说明当前需求为什么需要它。

## 附加约束

### 技术栈

| 层级 | 技术 | 版本要求 |
|------|------|----------|
| 桌面框架 | Tauri | v2 |
| 前端框架 | React + TypeScript | 18 + 5.x |
| UI 库 | MUI 5 + Tailwind CSS 3 | 最新稳定版 |
| 构建工具 | Vite | 5.x |
| Rust 异步 | tokio (full features) | 1.x |
| 序列化 | serde + serde_json | 1.x |
| 存储 | JSON 文件（~/.yt-dlp-sub-gui/） | N/A |
| 外部工具 | yt-dlp CLI | 最新稳定版 |

## 开发工作流

### TDD 循环

```
编写测试 → 展示给用户确认 → 测试失败（验证） → 实现最小代码 → 测试通过 → 重构
```

1. **编写测试**：基于规格说明或用户需求，先写出测试用例。
2. **确认测试**：在实现前将测试用例展示给用户，确保覆盖正确。（AI 代理可直接进入第 3 步）
3. **确认失败**：运行测试确认它们因缺少实现而失败。
4. **实现**：编写最小化代码使测试通过。
5. **重构**：测试通过后清理代码，消除重复，优化结构。

### 测试要求

- **Rust 单元测试**：每个 Service 中的公开函数 MUST 有对应的单元测试。
- **Rust 集成测试**：涉及 yt-dlp CLI 调用的核心路径 SHOULD 通过模拟或真实 CLI 测试（可在 CI 中通过特性门控跳过）。
- **前端测试**：关键用户交互（添加订阅、检查更新、设置修改）SHOULD 有集成测试。
- **测试运行**：`cargo test` 和 `npx tsc --noEmit` MUST 在提交前通过。

### 代码审查要点

- 是否有对应的失败测试？
- 是否引入了不必要的抽象？（YAGNI）
- 是否消除了重复代码？（DRY）
- 方案是否是最简单的可行解？（KISS）
- 新代码是否可测试？

## Governance

- 本章程是项目开发的最高指导原则，所有开发活动 MUST 遵守上述原则。
- 章程修订 MUST 通过文档更新、版本号升级和团队审查。
- 任何原则的例外 MUST 在相应 PR 中明确说明理由并获得批准。
- 违反不可协商原则（I. TDD）的代码 MUST 被拒绝合并。
- 复杂性决策（如引入新依赖、新抽象层）MUST 在 `plan.md` 的 Complexity Tracking 中说明理由。

**Version**: 1.0.0 | **Ratified**: 2026-05-31 | **Last Amended**: 2026-05-31
