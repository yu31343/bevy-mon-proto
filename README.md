# Rust项目：2D回合制精灵对战游戏原型系统

## 基于Rust语言与Bevy游戏引擎的开发实践

## 写在项目前

### 关于git的使用：

当前项目开发主要在 `develop` 分支。建议每个开发者创建一个自己的分支，如 `dev-name` 并拉取develop的最新内容，在自己的分支上进行修改。注意，做任何修改前 **请确认你的本地文件与当前develop分支的内容同步！**

你的分支当前阶段的修改完成后，请 **提交  pull requests** 合并到 `develop` 或提醒仓库拥有者进行合并更改。


### 项目注意事项：
本项目需要使用 bevy ，编译前 cargo 会自动下载
如果cargo的下载速度较慢，可以使用以下镜像源加速下载：

在linux： `~/.cargo/config.toml `文件或

Windows下：`C:\Users\你的用户名\.cargo\config.toml`中，(没有请自行创建该文件)
添加以下内容：

```toml
[source.crates-io]
registry = "https://github.com/rust-lang/crates.io-index"
replace-with = 'aliyun'
[source.aliyun]
registry = "sparse+https://mirrors.aliyun.com/crates.io-index/"
```
请注意，目前阿里云镜像仅支持稀疏索引配置，需要您的 cargo 版本 >=1.68。



另外，本项目默认开启了  [dynamic_linking](https://bevy.org/learn/quick-start/getting-started/setup/#dynamic_linking) 特性，并同时启用了 [performance optimizations](https://bevy.org/learn/quick-start/getting-started/setup/#compile-with-performance-optimizations)，以加速编译。

这使得编译生成的exe可执行文件无法独立运行，如果需要release正式的独立可执行文件，请注意移除该特性，

详情请自行阅读[bevy官方文档](https://bevy.org/learn/quick-start/getting-started/) 。



# 一、项目当前状态

本项目当前是一个基于 Rust + Bevy 的 2D 回合制精灵对战原型，已从早期 MVP 描述推进到较完整的“大厅 / 地图 / 队伍选择 / 本地战斗 / 局域网或中继 PVP / 日志导出”闭环。

核心开发方向已经从“单场战斗验证”扩展为：

- **数据驱动战斗内容**：技能、精灵、卡牌、元素克制、状态、反应、战斗规则等统一从 `assets/data/battle_data.ron` 加载。
- **双状态机战斗流程**：顶层 `GameState` 管理大厅、地图、队伍选择、战斗、结果；嵌套 `BattlePhase` 管理战斗初始化、回合开始、玩家回合、敌方回合、弃牌、结算、死亡处理。
- **本地与 PVP 共用战斗系统**：Vs AI、调试双方控制、PVP 模式都复用同一套战斗资源和 UI。
- **卡牌与队伍策略**：每场战斗使用共享抽牌堆 / 弃牌堆，卡牌可影响 AP、护盾、治疗、元素附着、反应、抽牌和战术整理。
- **结构化调试能力**：控制台日志、`StructuredBattleLog`、`ReplayEventLog`、`ActionTrace` 共同用于战斗复盘和问题定位。
- **Spine 动画表现**：战斗中可加载 Spine 精灵动画和 VFX，并根据战斗消息驱动表现层反馈。

---

# 二、使用方法

## 1. 环境准备

建议使用较新的 Rust stable 工具链，并从仓库根目录执行命令。

```bash
rustup update
cargo --version
```

本项目启动和测试依赖固定相对路径读取资源，请确保工作目录是项目根目录。

## 2. 资源要求

构建前需要满足：

- `assets/fonts/` 下至少存在一个 `.ttf`、`.otf` 或 `.ttc` 字体文件。
- `assets/data/battle_data.ron` 存在且可被解析。

`build.rs` 会按文件名排序选择第一个支持的字体并嵌入 UI 字体资源；如果没有字体文件，构建会失败。

## 3. 常用命令

```bash
# 快速编译检查
cargo check

# 格式化代码
cargo fmt --all

# 运行 Clippy，并将警告视为错误
cargo clippy --all-targets -- -D warnings

# 运行全部测试
cargo test

# 运行单个测试
cargo test <test_name>

# 运行单个测试并显示 println!/debug 输出
cargo test <test_name> -- --nocapture

# 从仓库根目录启动游戏
cargo run

# 构建 release 版本
cargo build --release
```

> 注意：当前 `Cargo.toml` 默认启用了 Bevy 的 `dynamic_linking` 特性以加快开发迭代。该模式下 `cargo run` 正常可用，但生成的可执行文件不是独立发布包。需要发布独立 release 时，请先移除 `dynamic_linking`，再执行 `cargo build --release`。

---

# 三、游戏流程说明

## 1. 大厅

游戏启动后进入大厅。大厅可进入：

- VS AI 队伍选择
- 地图探索
- PVP 大厅
- 精灵图鉴 / 调试入口

## 2. 地图探索

地图模式提供当前原型阶段的简单 overworld：

- 玩家可移动角色。
- 地图上会生成怪物标记。
- 点击地图怪物后，会记录该怪物作为敌方目标，并进入队伍选择。

## 3. 队伍选择

队伍选择支持三种入口模式：

- **Vs AI**：玩家只选择我方队伍，敌方由地图目标或 AI 自动选择生成。
- **Debug**：开发调试模式，玩家可先选我方，再选敌方，方便复现战斗问题。
- **PVP**：本地提交队伍后等待远端队伍，通过 PVP 协议进入战斗。

## 4. 战斗

战斗按回合进行，核心流程为：

1. 初始化双方队伍、手牌、AP、共享卡堆和随机种子。
2. 回合开始：双方抽牌、获得 AP、清理回合级卡牌效果。
3. 根据有效速度决定先手，速度相同时交替先手。
4. 玩家 / 敌方依次行动：使用技能、使用卡牌、弃牌换 AP、换人或结束回合。
5. 若手牌超过保留上限，进入强制弃牌阶段。
6. 检查倒下、自动或手动换人、胜负结果。
7. 进入结果页，可返回大厅并重置选择状态。

## 5. PVP

PVP 支持：

- 局域网直连 TCP。
- 中继服务器模式。
- 协议版本校验。
- 战斗数据 hash 校验。
- Host 权威战斗：客户端发送意图，Host 应用并广播反馈 / 快照。

当协议版本或双方战斗数据不一致时，握手会失败并在 UI 与控制台日志中提示原因。

---

# 四、架构概览

## 1. 插件入口

`src/main.rs` 负责注册顶层状态和插件。主要插件包括：

- `DataPlugin`：加载战斗数据。
- `UiPlugin`：共享 UI 主题、字体、战斗 UI 生命周期。
- `LobbyPlugin`：大厅入口。
- `MapPlugin`：地图探索。
- `PvpPlugin`：PVP 大厅、网络、同步。
- `TeamSelectionPlugin`：队伍选择。
- `BattlePlugin`：战斗状态机和战斗系统。
- `SpineAnimPlugin`：Spine 动画和 VFX。

## 2. 数据驱动

战斗数据集中在：

```text
assets/data/battle_data.ron
```

当前数据包括：

- 战斗规则
- 公式规则
- 元素克制矩阵
- 状态定义
- 元素反应定义
- 技能定义
- 精灵原型
- 卡牌定义
- 初始牌组顺序

启动时会插入 `BattleDbs`、`MonsterPool`、`CardDeck`、`BattleRules`、`BattleFormulaRules`、`BattleRulesBundle`、`BattleDataStatus` 等资源。

如果读取、解析或校验失败，系统会插入 fallback 资源，并把失败原因写入 `BattleDataStatus.error`。下游系统应以该资源作为权威失败信号。

## 3. 战斗资源与消息

战斗参与者是 Bevy ECS 实体，通过组件组合描述：

- `Combatant`
- `Stats`
- `SkillList`
- `SkillCount`
- `Shield`
- `ElementAura`
- `StatusBoard`
- `InBattle`

核心资源包括：

- `PlayerTeam` / `EnemyTeam`
- `TurnContext`
- `ActionPoints`
- `Hand`
- `CardPiles`
- `PendingBoosts`
- `BattleResult`
- `PendingKoResolution`
- `StructuredBattleLog`
- `ReplayEventLog`
- `ActionTrace`

核心战斗反馈通过 Bevy message 传递，例如：

- `BattleEvent`
- `BattleTraceEvent`
- `BattleStateEvent`
- `BattleLifecycleEvent`
- `BattleFormulaEvent`
- `BattleStatusEvent`

这使得核心逻辑、UI、动画表现、日志系统之间保持相对解耦。

---

# 五、控制台日志与调试

当前项目已接入统一控制台日志模块 `src/console_log.rs`。默认日志强调人类可读，适合直接观察一场战斗流程。

## 1. 默认会输出的内容

默认运行 `cargo run` 时，控制台会输出：

- 数据加载成功 / 失败摘要。
- 队伍选择结果。
- 地图点击怪物。
- 战斗初始化摘要：模式、随机种子、双方队伍、规则。
- 初始手牌和每回合抽牌。
- 回合先手、速度、AP、手牌数量。
- 技能、卡牌、弃牌、伤害、治疗、护盾、元素附着、反应、换人、倒下、胜负。
- PVP 建房、连接、握手、失败、断线摘要。
- AI 实际选择：技能、换人、结束回合、辅助使用增益卡。
- Spine 动画资源缺失 / 加载失败等表现层警告。

日志示例：

```text
[data] [ok] battle_data.ron 加载完成：skills=... cards=... monsters=... statuses=... reactions=... deck=... element_matrix=config
[selection] 玩家选择：...
[battle] [battle-init] 模式=PlayerVsAi；随机种子=...；我方=[...]；敌方=[...]
[cards] [battle-init] 初始手牌：我方=[...]；敌方=[...]；牌堆 ... 张；弃牌 ... 张
[battle] [round 1] 先手=Player；玩家Spd=...；敌方Spd=...；玩家AP=...；敌方AP=...
[ai] [round 1][enemy] 选择技能：...；槽位=...；得分=...；AP ... -> ...
```

## 2. 环境变量

可通过环境变量打开更详细的 debug 日志：

| 环境变量 | 作用 |
|---|---|
| `BEVY_MON_LOG_DEBUG=1` | 打开战斗 debug、公式、状态、状态快照、ActionTrace，并联动卡堆 / PVP / AI 细节。 |
| `BEVY_MON_LOG_BATTLE_DEBUG=1` | 只打开战斗结构化 debug 输出。 |
| `BEVY_MON_LOG_CARDS=1` | 打开卡堆细节，例如牌堆耗尽、弃牌堆重洗、基础牌组重建；卡牌效果抽牌会默认记录来源和抽到的卡名。 |
| `BEVY_MON_LOG_PVP=1` | 打开 PVP intent、snapshot、battle feedback、data hash 等同步细节。 |
| `BEVY_MON_LOG_AI=1` | 打开完整 AI 技能候选、评分构成、候选换人等决策细节。 |
| `BEVY_MON_LOG_SPINE=1` | 打开 Spine 动画生成、UI ready 等表现层细节。 |

## 3. 使用示例

Linux / macOS：

```bash
BEVY_MON_LOG_DEBUG=1 cargo run
BEVY_MON_LOG_CARDS=1 cargo run
BEVY_MON_LOG_PVP=1 cargo run
BEVY_MON_LOG_AI=1 cargo run
BEVY_MON_LOG_SPINE=1 cargo run
```

Windows PowerShell：

```powershell
$env:BEVY_MON_LOG_DEBUG="1"; cargo run
$env:BEVY_MON_LOG_CARDS="1"; cargo run
$env:BEVY_MON_LOG_PVP="1"; cargo run
$env:BEVY_MON_LOG_AI="1"; cargo run
$env:BEVY_MON_LOG_SPINE="1"; cargo run
```

Git Bash / Bash on Windows：

```bash
BEVY_MON_LOG_DEBUG=1 cargo run
```

## 4. 结构化日志与导出

运行期还维护：

- `BattleLog`：UI 和控制台可读的短文本日志。
- `StructuredBattleLog`：阶段、状态快照、公式摘要等结构化调试信息。
- `ReplayEventLog`：可导出的战斗事件序列。
- `ActionTrace`：行动链路追踪，记录回合、阵营、动作和细节。

结果页可导出 replay 与 action trace，用于复盘战斗顺序、排查状态或卡牌效果问题。

---

# 六、开发状态与后续方向

## 已具备

- 大厅、地图、队伍选择、战斗、结果返回的基本闭环。
- Vs AI、Debug 双方控制、PVP 三类战斗入口。
- 数据驱动技能、状态、反应、精灵、卡牌和规则。
- 共享抽牌堆 / 弃牌堆与 per-battle 洗牌种子。
- AP、手牌上限、强制弃牌、战术整理等卡牌资源系统。
- 元素附着、元素克制、元素反应、风扩散、状态 tick、属性等级修正。
- PVP 协议握手、数据 hash 校验、host 权威 intent / snapshot 同步。
- Spine 战斗动画与 VFX 原型。
- 控制台可读日志、结构化日志、ActionTrace 和导出能力。

## 仍可深化

- 更完整的 AI 战术策略，而不仅是当前评分启发式。
- PVP 双端日志链路的手动实测与可视化调试工具。
- 地图探索、养成、捕捉、图鉴等战斗外玩法。
- 更完整的战斗 replay 回放与自动化复盘工具。

---

# 七、测试与质量要求

提交修改前建议至少运行：

```bash
cargo fmt --all
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
```

如果修改了战斗数据或战斗公式，建议额外：

- 运行数据解析相关测试。
- 进入 Vs AI 战斗手动验证一局。
- 打开 `BEVY_MON_LOG_DEBUG=1` 观察公式、状态、ActionTrace 是否符合预期。
- 如果改动涉及 PVP，至少验证 host/client 握手和数据 hash 一致性。
