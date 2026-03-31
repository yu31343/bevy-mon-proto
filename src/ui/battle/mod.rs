//! 战斗界面 UI（按领域拆分的模块入口）。

pub(crate) mod components;
pub(crate) mod fx;
pub(crate) mod layout;
pub(crate) mod resources;
pub(crate) mod systems;
pub(crate) mod theme;

mod plugin;

use bevy::prelude::*;

/// 注册战斗 UI（系统与资源）。
///
/// 注意：此函数的语义应与旧版 `UiPlugin` 保持一致。
pub(crate) fn register(app: &mut App) {
    plugin::register(app);
}

