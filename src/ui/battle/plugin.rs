//! 将战斗 UI 挂载到 Bevy `App`。

use bevy::prelude::*;

pub(crate) fn register(app: &mut App) {
    // 先桥接到旧实现：后续步骤会把实现迁移进 `ui/battle/*`。
    crate::ui::register_legacy_battle_ui(app);
}
