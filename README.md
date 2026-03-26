# Rust项目：2D回合制精灵对战游戏原型系统

## 基于Rust语言与Bevy游戏引擎的开发实践

## 写在项目前

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



# 一、 项目核心开发内容 (MVP 阶段)

核心开发围绕“数据-逻辑-表现”三个维度展开，确保游戏闭环的完整性。

## 1. 核心架构与数据驱动
* **ECS 实体组件系统：** 将精灵（Entity）拆解为基础属性（Health, Attack, Speed）、技能列表（SkillList）和视觉状态（SpriteBundle）等独立组件。
* **数据解耦：** 建立基于配置（如 TOML/JSON）的精灵原型库，实现“数据定义属性，代码驱动逻辑”的解耦模式。

## 2. 回合制战斗逻辑状态机
* **行动流控：** 基于 Bevy `States` 实现战斗周期的严格管理：
    * **判定阶段：** 依据 `Speed` 属性计算行动顺序切片。
    * **指令阶段：** 等待玩家输入或 AI 决策。
    * **结算阶段：** 计算属性变化、触发技能特效、判定胜负。
* **伤害演算：** 实现标准的减法或乘法伤害公式，支持防御减伤与属性克制。

## 3. UI 交互与视觉呈现
* **动态 UI 系统：** 实时同步实体属性至 UI 界面（如血条 `ProgressBar`、技能菜单）。
* **战斗表现：** 利用 `bevy_tweening` 实现精灵的攻击平移、受击震动及数值漂浮文字。

## 4. 基础 AI 行为
* **决策模型：** 实现基于权重或简单规则的 AI（如：生命值低于 20% 优先回血，否则执行最高伤害攻击）。

---

# 二、 潜在拓展功能 (进阶阶段)

在核心原型稳定后，可根据开发周期逐步引入以下模块以提升游戏深度：

* **养成系统：** 引入经验值（XP）与等级（Level）组件，实现战斗后的属性成长。
* **探索机制：** 基于 `bevy_ecs_tilemap` 构建 2D 地图，实现野外遇敌与精灵捕捉逻辑。
* **社交与竞技：** 利用 Rust 优秀的异步网络库（如 `tokio` / `renet`）尝试基础的局域网 PVP 对战。
* **表现增强：** 增加粒子特效（技能释放）与骨骼动画（精灵待机动作）。

---

# 三、 项目特色与创新点

本项目不仅是简单的复刻，更在技术栈应用与设计理念上具备独特性：

## 1. 技术栈的前沿性与高性能
* **内存安全保证：** 利用 Rust 的所有权机制，从底层根除空指针及资源竞态问题，确保战斗系统在高并发（如大规模 AI 运算）下的稳定性。
* **数据驱动的 ECS 范式：** 相比传统 OOP 游戏开发，本项目采用 Bevy 的 ECS 架构。这种**“组合优于继承”**的设计使得精灵的功能扩展极其灵活——只需为实体挂载一个新的 `Component`，即可为其增加全新的战斗机制（如“反击”或“光环”）。

## 2. 极致的模块化设计
* **插件化架构：** 将战斗、AI、UI 各自封装为 Bevy `Plugin`。这意味着战斗逻辑可以无缝迁移到不同的场景（如从 2D 切换到 3D），或者方便地进行单元测试。

## 3. 策略深度的潜力
* **基于“速度流”的动态回合：** 不同于固定的你一刀我一刀，系统支持基于速度属性的动态排序，为后续引入“拉条”、“控制速度”等高级策略打下基础。

## 4. 开发者友好型工具链
* **热重载支持：** 结合 Rust 的编译特性与 Bevy 的资源管理，项目可实现不重启程序即可修改精灵数值或技能效果，极大提升了策划调优的效率。

