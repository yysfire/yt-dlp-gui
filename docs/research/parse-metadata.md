# 研究：yt-dlp 能否可靠改写 `title` 字段（issue #20）

**结论（verdict）：DO —— 但机制不是 `--parse-metadata`，而是 `--replace-in-metadata`；且必须配套文件名去重，否则会造成文件名碰撞。**

- 调研日期：2026-10-10
- 实测环境：`yt-dlp 2026.06.09`（scoop 安装，`yt-dlp.exe`），Windows 11，全部离线（`--load-info-json` + `--simulate`）
- 上游依据：`yt-dlp/yt-dlp@master` 的 `README.md`、`yt_dlp/options.py`、`yt_dlp/postprocessor/metadataparser.py`、`yt_dlp/YoutubeDL.py`

---

## 0. TL;DR

| 问题 | 答案 |
|------|------|
| 1. `--parse-metadata FROM:TO[:REPLACEMENT]` 的精确语义 | **提问的前提有误**：不存在 `REPLACEMENT` 第三段。它只有 `FROM:TO` 两段，且 `TO` 不是「替换串」而是「目标字段名 / 命名捕获正则 / 输出模板」 |
| 2. 能否只改 `title` | **能**，但要用 `--replace-in-metadata title <regex> <replace>`；`--parse-metadata` 也能写title，但不安全（见§2.3） |
| 3. flat-playlist 与全元数据路径是否一致 | **一致**。两者共用 `pre_process`，改写发生在格式选择之前 |
| 4. 能否与 `--load-info-json`（离线预览）配合 | **能**，实测通过 |
| 5. 是稳定的用户功能还是内部机制 | **正式用户功能**：README 有独立顶层章节 `MODIFYING METADATA`，`--replace-in-metadata` 还有专门示例。上游漂移风险低 |
| 6. 有无更合适的官方机制 | **有，且现成**：`--replace-in-metadata`。本项目的需求应当直接用它，不必自己拼 `--parse-metadata` |

**给本项目的落地建议**：用 `--replace-in-metadata "title" "\s*\([0-9]+:[0-9]+(?::[0-9]+)?\)\s*$" ""`，
**并且**把输出模板从 `%(title)s.%(ext)s` 改为带 `%(id)s` 的形式以规避碰撞（见 §5）。

---

## 1. 官方文档定位（问题 5）

`--help` 与 `README.md` 均把它列为**顶层用户选项**，并指向一个专门的顶层章节：

```
--replace-in-metadata [WHEN:]FIELDS REGEX REPLACE
                        Replace text in a metadata field using the given
                        regex. This option can be used multiple times.
                        Supported values of "WHEN" are the same as that of
                        --use-postprocessor (default: pre_process)
```

`README.md` 第 1776 行起有独立章节 `# MODIFYING METADATA`，并在
`## Modifying metadata examples`（第 1818 行）里给出**直接对口本项目的示例**：

```bash
# Replace all spaces and "_" in title and uploader with a `-`
$ yt-dlp --replace-in-metadata "title,uploader" "[ _]" "-"
```

而且 `--metadata-from-title` 这个历史兼容项在第 2422 行被明确标注为
`--parse-metadata "%(title)s:FORMAT"` 的别名 —— 说明这是一等公民能力，不是内部实现细节。

**与 `PROGRESS_TEMPLATE` 的对比（上游漂移风险）**：`PROGRESS_TEMPLATE`
（`src-tauri/src/services/ytdlp.rs:60-68`）依赖的是 `%(progress._percent)s` 这类
**下划线前缀的内部字段**，README 只把它们列在
[EXPERIMENTAL FEATURES](https://github.com/yt-dlp/yt-dlp#experimental-features) 一类的实验性说明里，
且历史上 yt-dlp 改字段名即静默失效。相比之下 `--replace-in-metadata` 走的是
**稳定的 postprocessor 参数接口 + 纯 Python `re`**，yt-dlp 没有动机去改它的语义。

### 实现位置（源码级证据）

`yt_dlp/options.py:1735-1739`：

```python
'--replace-in-metadata',
dest='parse_metadata', metavar='[WHEN:]FIELDS REGEX REPLACE', nargs=3, **when_prefix('pre_process'),
```

注意 `dest='parse_metadata'`：**两个 CLI 选项共用同一个 dest**，实现是同一个
postprocessor `MetadataParserPP`（`yt_dlp/postprocessor/metadataparser.py`）的两个 action：

| CLI | 类 | action |
|-----|-----|--------|
| `--parse-metadata FROM:TO` | `MetadataFromFieldPP` | `Actions.INTERPRET` |
| `--replace-in-metadata FIELDS REGEX REPLACE` | `MetadataParserPP` | `Actions.REPLACE` |

`README.md` 明确说明改写发生在**格式选择、后处理之前**：

> **Note**: Metadata modification happens before format selection, post-extraction and other
> post-processing operations. Some fields may be added or changed during these steps, overriding
> your changes.

并且改写后的字段**可以直接用于输出模板**：

> any field thus created can be used in the [output template](#output-template)

---

## 2. 精确语义（问题 1）

### 2.1 `--parse-metadata FROM:TO` —— **两段，不是三段**

提问里写的 `FROM_REGEX:TO_REGEX[:REPLACEMENT]` 中，第三段 `REPLACEMENT` **不存在**。
实测证据：

```
$ yt-dlp --parse-metadata "title/Episode/Episode/"
ERROR: --parse-metadata title/Episode/Episode/ is invalid; it should be FROM:TO, not 'title/Episode/Episode/'
```

源码（`metadataparser.py:106-116`）：

```python
class MetadataFromFieldPP(MetadataParserPP):
    @classmethod
    def to_action(cls, f):
        match = re.match(r'(?s)(?P<in>.*?)(?<!\\):(?P<out>.+)$', f)
        if match is None:
            raise ValueError(f'it should be FROM:TO, not {f!r}')
        return (cls.Actions.INTERPRET, match.group('in').replace('\\:', ':'), match.group('out'))
```

要点：
- 分隔符是**单个冒号 `:`**，且**从右侧最后一个未转义的冒号切分**；
- 正则里要写字面冒号必须写成 `\:`（`(?<!\\)` 是负向后视，转义冒号不参与切分）；
- `FROM` 可为字段名或输出模板；`TO` 可为①单个字段名②含**命名捕获组** `(?P<x>...)` 的正则③`%(field)s` 形式的输出模板。

`TO` 为正则时的写入规则（`metadataparser.py:66-81`）：**只写命名捕获组**，
且**不匹配就什么都不做**：

```python
match = out_re.search(data_to_parse)
if match is None:
    self.to_screen(f'Could not interpret {inp!r} as {out!r}')
    return
for attribute, value in filter_dict(match.groupdict()).items():
    info[attribute] = value
```

### 2.2 `--replace-in-metadata FIELDS REGEX REPLACE` —— 真正的「条件改写」

`FIELDS` 是逗号分隔的字段名列表；`REPLACE` 是 `re.subn` 的替换串（支持 `\1` 反向引用）。

源码（`metadataparser.py:83-101`）—— 语义**恰好符合本项目需求**：

```python
@function_with_repr
def replacer(self, field, search, replace):
    def f(info):
        val = info.get(field)
        if val is None:
            self.to_screen(f'Video does not have a {field}')
            return
        elif not isinstance(val, str):
            self.report_warning(f'Cannot replace in field {field} since it is a {type(val).__name__}')
            return
        info[field], n = search_re.subn(replace, val)
        ...
    search_re = re.compile(search)
```

| 情况 | 行为 |
|------|------|
| `TO_REGEX` 匹配 | 用 `REPLACE` 替换匹配段；`REPLACE` 为空串即**删除** |
| 不匹配 | **原样保留**（`subn` 返回 n=0，`info[field]` 不变） |
| 字段不存在 | 打印 `Video does not have a <field>` 并跳过 |
| 字段非字符串（如 `tags` 是 list） | `report_warning` 并跳过 |

实测（`--verbose`，探针 title = `Episode 3: The End (11:30)`）：

```
[debug] Replacing all '\s*\([0-9]+:[0-9]+(?::[0-9]+)?\)\s*$' in title with ''
[MetadataParser] Changed title to: Episode 3: The End
```

无匹配的视频保持原样：

```
[MetadataParser] Did not find '\s*\([0-9]+:[0-9]+(?::[0-9]+)?\)\s*$' in title
  TITLE=[Plain Title No Duration]
```

### 2.3 为什么不能用 `--parse-metadata` 做这件事（重要）

`--parse-metadata` 是**无条件覆盖**：它不匹配时不写，但一旦「匹配上了不该匹配的」就会破坏数据。
危险点在 `FROM` 字段缺失时：`evaluate_outtmpl` 会渲染出字符串 `NA`，而 `.+` 会**匹配 `NA`**。
实测：

```
$ yt-dlp --parse-metadata "episode:title"
TITLE=[NA]     ← 源字段 episode 不存在，却把 title 覆盖成了字符串 "NA"
```

```
$ yt-dlp --parse-metadata "title:(.*):X"
TITLE=[Episode 3: The End (11:30)]   ← 不匹配，但 source 是 title 本身，TO 无命名组，未写入
```

而 `--parse-metadata "description:(?P<title>.+)"` 会**把整段 description 写进 title**：

```
TITLE=[A description with a colon: and a duration (11:30) inside.]
```

结论：`--parse-metadata` 无法表达「只删掉标题末尾的时长后缀，且其余部分逐字保留」——
它只能「从别处解析出一个新值」。要改写title 自身，`--replace-in-metadata` 是唯一正确选择。

---

## 3. 字段限定（问题 2）

**能只改 `title`。** 实测三条：

```bash
# 只改 title
--replace-in-metadata "title" '\s*\([0-9]+:[0-9]+(?::[0-9]+)?\)\s*$' ""
```

```
TITLE=[Episode 3: The End]              ← 已改写
DESC=[A description with a colon: and a duration (11:30) inside.]   ← 未动
UPL=[Chan: Nel]                          ← 未动
TAGS=[['tag:one', 'dur:11:30']]          ← 未动
```

多字段语法也可用（`FIELDS` 逗号分隔）：

```bash
--replace-in-metadata "title,uploader,description" "([0-9]+):([0-9]+)" "<\1h\2m>"
```
```
T=[Episode 3: The End (<11h30m>)]  U=[Chan: Nel]  D=[A description with a colon: and a duration (<11h30m>) inside.]
```
（注意 `uploader` 是 `Chan: Nel`，不含 `11:30`，所以未被替换 —— 符合预期。）

对非字符串字段（`tags`）会安全跳过并告警，不会崩溃。

---

## 4. flat-playlist 一致性 + `--load-info-json` 兼容（问题 3、4）

### 4.1 `--load-info-json` 兼容 —— 问题 4：能

所有实测都走 `--load-info-json ./probe.json --simulate`，改写如期生效。
最小载荷（`id`/`title`/`ext`/`url`/`extractor`/`extractor_key`）即可工作。

### 4.2 flat-playlist 一致性 —— 问题 3：一致

**源码依据**：`process_video_result`（`YoutubeDL.py:2835`）是单视频与 flat-playlist 条目的
**共同**路径，flat 条目经 `__process_iterable_entry`（`YoutubeDL.py:2199-2201`）
直接调用同一个 `process_ie_result`。而 `pre_process`（改写发生处）
在 `YoutubeDL.py:3037`，位于 `process_video_result` 内、`post_extract`（3042）之前，
**不分叉**。README 也明说改写发生「before format selection」。

**实测**：用 flat-playlist 真实返回的条目形状（只有 `id`/`title`/`url`/`duration`，无 `ext`/`formats`）：

```
### 不带改写
  T=[Flat Item (11:30)] ID=[e1]
### 带 --replace-in-metadata
  T=[Flat Item] ID=[e1]
```

> 局限说明：用 `--load-info-json` 喂一个 `_type:playlist` 容器时，yt-dlp 会因
> `[generic]` 提取器无法联网而丢弃其entries（`There are no entries`），故上述一致性
> 是通过「直接加载扁平条目」而非「加载 playlist 容器」验证的。结合源码路径可确认
> 结论成立，但严格意义上未做真实YouTube 频道的端到端验证。

---

## 5. 关键风险：文件名碰撞（verdict 的必要论证）

**合法冒号会被保留** —— 这正是问题所在。`Episode 3: The End` 中的冒号**不会**被
`--replace-in-metadata` 触碰（正则只匹配末尾的 `(mm:ss)`），实测：

```
TITLE=[Episode 3: The End]  FILE=[...\out\Episode 3： The End.mp4]
```

注意 `：` 是 yt-dlp 在 Windows 上对非法字符 `:` 的替换（全角冒号），
与 `AGENTS.md` 记录的 `[0-9]+(?::[0-9]+)+` → `_` 是两条不同的规则：
- `Video 11:30`（数字时间）→ `Video 11_30`（冒号变下划线）
- `Episode 3: The End`（文本冒号）→ `Episode 3： The End`（冒号变全角）

**碰撞已实测证实**。构造两个视频：

| 探针 | 原始 title | 改写后 | 落盘路径 |
|------|-----------|--------|---------|
| `p1.json` | `Episode 3: The End (11:30)` | `Episode 3: The End` | `...\Episode 3： The End.mp4` |
| `p2.json` | `Episode 3: The End`（无后缀） | `Episode 3: The End` | `...\Episode 3： The End.mp4` |

**两者落到完全相同的绝对路径。** 后果按 `AGENTS.md` 的记录是严重的：
输出模板 `%(title)s.%(ext)s` 没有订阅判别符，而本项目的核心不变量是
「一个 `video_id` ↔ 一条记录 ↔ **一个文件**」。两个不同 `video_id` 抢同一路径会
直接破坏该不变量（记录指向同一个 `file_path`，先下载的被后下载的覆盖）。

### 缓解方案（若要DO，必须一并实施）

把模板改成含 `%(id)s` 的形式，碰撞即被 `video_id` 消除：

```
- o "%(title)s.%(ext)s"                              #现状，有碰撞风险
- o "%(title)s [%(id)s].%(ext)s"                     # 保守：保留可读性 + 唯一性
```

注意 `%(id)s` 是 YouTube 的 11 位视频 ID，天然唯一；B站需另行确认
（`services/ytdlp.rs` 已处理双平台，`id` 语义可能不同，实施前需复核 B 站 ID 是否稳定唯一）。

---

## 6. 其它候选机制评估（问题 6）

| 机制 | 能否在下载前改写 title | 评价 |
|------|---------------------|------|
| **`--replace-in-metadata`** | **能**，`pre_process` 阶段 | ✅ **推荐**。README 顶层章节 + 专门示例，语义恰为「条件替换」 |
| `--parse-metadata` | 能，但只能「从别处解析出新值」 | ⚠️ 能用但易误伤（源字段缺失时写入 `NA`），且无法表达「保留其余部分」 |
| `--exec` | ❌ | 默认 `when=after_move`，**下载并落盘之后**才执行；用法是「对已下载文件做事」，不是「决定文件名」。且需 shell 转义（本项目还要额外处理 AppImage/PYTHONHOME 之外的环境问题），成本远高于收益 |
| `--print` | ❌ | 只**读**并输出模板字段，不改写元数据 |
| postprocessor hooks | 能（写自定义 PP） | 需要写 Python 插件并分发，与本项目「零 Python 依赖、单 exe 交付」的形态冲突 |
| `--embed-metadata` 的 `meta_` 前缀 | ❌（对我们无意义） | 只改**嵌入到媒体文件内部**的标签，不影响输出文件名 |

结论：**没有比 `--replace-in-metadata` 更合适的官方机制**。它是为此场景设计的。

---

## 7. 稳定性评估（问题 5 结论）

**风险等级：低。** 依据：

1. **文档承诺**：README 有独立顶层章节 + 独立示例小节，不是「顺手提一句」的选项。
2. **无内部字段依赖**：参数是 `FIELDS REGEX REPLACE` 三个纯字符串 + 一个 `nargs=3`，
   不含 `%(progress._percent)s` 那类下划线内部字段。yt-dlp 若要破坏它得改 CLI 参数形状，
   属于明确的 breaking change，而非静默改名。
3. **实现极简**：整个 `replacer` 只有约 15 行，语义就是 `re.subn`。
4. **失败可见**：不匹配会打 `Did not find ... in <field>`，字段缺失会打
   `Video does not have a <field>`，非字符串字段会 `report_warning`。
   即使上游行为变化，也**不会静默产出错误文件名**。

对比 `AGENTS.md` 记录的 `PROGRESS_TEMPLATE` 事故：那是真正的静默失效
（`_percent_str` vs `percent` 字段名变化 → 所有进度行解析失败 → 进度条空白）。
本机制的风险量级完全不同。

**实施建议**：即便如此，仍应在 `services/ytdlp.rs` 中把改写参数**集中在常量里**
（与 `PROGRESS_TEMPLATE` 同等对待），并在 CI/手动验证中保留一条
「带时长的标题 → 文件名不含时长」的断言。

---

## 8. 我未能回答的问题（明确记录）

| 问题 | 状态 | 原因 |
|------|------|------|
| **真实 YouTube / B站 端到端行为** | ❌ 未验证 | 全程离线（`--load-info-json`）。`--flat-playlist` 容器路径因无网络而丢弃 entries，改用「直接加载扁平条目」替代验证（见 §4.2） |
| **`--replace-in-metadata` 在真实站点上的字段完整度** | ⚠️ 部分推断 | 无法确认 YouTube 提取器返回的 `title` 是否**总是**含 `(mm:ss)` 后缀；部分视频可能把时长放在别处。此结论基于 yt-dlp 标题语义的一般知识，未经联网核实 |
| **B站 `%(id)s` 是否稳定唯一** | ❌ 未验证 | 纯离线，无法拉取 B站 条目确认 ID 形态。§5 的缓解方案若采纳，此项需先验证 |
| **yt-dlp 上游历史（git blame）是否有过语义变更** | ❌ 未查 | 只核对了 `master` 当前状态与其 README，未追溯提交历史。结论「稳定」基于当前 API 形态 + 文档地位，而非历史记录 |
| **`--parse-metadata` 的 `WHEN` 阶段（`pre_process` / `after_filter` / `video` / `before_dl`）在本场景下是否有差异** | ❌ 未逐一验证 | 只测了默认 `pre_process`。README 说改写「before format selection」，理论上各阶段行为一致，但未实测 |

---

## 9. 给 issue #20 的最终建议

**DO（机制可靠，功能可做）**，但落地条件是三项一起做，缺一不可：

1. 用 **`--replace-in-metadata`**（不是 `--parse-metadata`）改写 `title`；
2. 输出模板加入 **`%(id)s`** 消除文件名碰撞（§5）；
3. 把改写参数**收敛为 `services/ytdlp.rs` 的常量**，并加一条回归断言。

如果**只**做第 1 项而不做第 2 项，则结论翻转为 **DON'T** —— 因为碰撞会破坏
「一个 `video_id` ↔ 一个文件」这条项目核心不变量，比时长后缀本身更糟。
若团队判定「改文件名格式」不在本期范围内，也应 **DON'T（本期不做）**，
因为半吊子实现的净损害为负。