# 领域文档

各工程技能在探索本代码库时，应如何消费本仓库的领域文档。

## 探索前先读这些

- 仓库根目录的 **`CONTEXT.md`**，或
- 仓库根目录的 **`CONTEXT-MAP.md`**（若存在）：它指向每个上下文各自的 `CONTEXT.md`。读取与当前主题相关的每一个。
- **`docs/adr/`**：读涉及你即将改动区域的 ADR。多上下文仓库中，还要检查 `src/<context>/docs/adr/` 里的上下文级决策。

若上述文件不存在，**静默继续**。不要提示缺失，也不要主动建议创建。`/domain-modeling` 技能（经 `/grill-with-docs` 与 `/improve-codebase-architecture` 触达）会在术语或决策真正被确定时按需创建。

## 文件结构

单上下文仓库（大多数仓库）：

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多上下文仓库（根目录存在 `CONTEXT-MAP.md`）：

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          ← 系统级决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← 上下文级决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用术语表的词汇

当你的输出提到某个领域概念（issue 标题、重构提案、假设、测试名），使用 `CONTEXT.md` 中定义的术语。不要漂移到术语表明确回避的同义词。

若需要的概念尚未进入术语表，这是个信号：要么你在发明项目不使用的语言（重新考虑），要么存在真实缺口（记下来交给 `/domain-modeling`）。

## 标记 ADR 冲突

若你的输出与既有 ADR 矛盾，明确提出来，而不是静默覆盖：

> _与 ADR-0007（事件溯源订单）矛盾，但值得重开，因为……_
