use bevy::prelude::*;

use crate::battle::{
    ElementAura, EnemyTeam, InBattle, PlayerTeam, Shield, Side, Stats, StatusBoard, Team,
    UiControlSide,
};
use crate::data::BattleFormulaRules;

use super::super::components::*;
use super::super::layout::{
    BENCH_BG, BENCH_BG_DEAD, BENCH_BG_HOVER, BENCH_BG_PRESSED, DEAD_MEMBER_BORDER,
};
use super::super::theme::UiTheme;

const BENCH_SHIELD_BAR_WIDTH: f32 = 92.0;
const SWITCH_SHIELD_BAR_WIDTH: f32 = 220.0;

/// 阵亡精灵名字的冷灰暗色，替代“已倒下”文字的视觉提示。
const DEAD_NAME_COLOR: Color = Color::srgba(0.46, 0.43, 0.43, 0.90);

/// 计算待机位的显示顺序：存活精灵优先靠前、阵亡精灵沉底，内部保持原相对顺序。
///
/// 返回 `display_order`，`display_order[display_position]` 为该展示位应显示的队伍索引。
/// 仅用于 UI 排版（把待机卡片按此顺序映射到队伍成员），不改动队伍真实顺序，
/// 避免影响 active_index / 技能槽位 / 5-7 切换键。
pub(crate) fn bench_display_order<F: Fn(Entity) -> bool>(team: &Team, is_dead: F) -> Vec<usize> {
    let mut alive: Vec<usize> = Vec::new();
    let mut dead: Vec<usize> = Vec::new();
    for (i, &entity) in team.combatants.iter().enumerate() {
        if is_dead(entity) {
            dead.push(i);
        } else {
            alive.push(i);
        }
    }
    alive.extend(dead);
    alive
}

fn shield_pct(stats: &Stats, shield: &Shield) -> f32 {
    if stats.max_hp > 0 {
        ((shield.0.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn shield_value_text(value: i32) -> String {
    if value > 0 {
        value.to_string()
    } else {
        String::new()
    }
}

fn apply_bench_shield_track(node: &mut Node, visibility: &mut Visibility, pct: f32) {
    let has_shield = pct > 0.0;
    node.width = Val::Px(if has_shield {
        BENCH_SHIELD_BAR_WIDTH * pct / 100.0
    } else {
        0.0
    });
    node.display = if has_shield {
        Display::Flex
    } else {
        Display::None
    };
    *visibility = if has_shield {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn apply_switch_shield_track(node: &mut Node, visibility: &mut Visibility, pct: f32) {
    let has_shield = pct > 0.0;
    node.width = Val::Px(if has_shield {
        SWITCH_SHIELD_BAR_WIDTH * pct / 100.0
    } else {
        0.0
    });
    node.display = if has_shield {
        Display::Flex
    } else {
        Display::None
    };
    *visibility = if has_shield {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn effective_stat_value(
    stats: &Stats,
    stat: StatStageModifierKind,
    formula_rules: &BattleFormulaRules,
) -> i32 {
    match stat {
        StatStageModifierKind::Atk => {
            super::super::helpers::effective_atk_value(stats, formula_rules)
        }
        StatStageModifierKind::Def => {
            super::super::helpers::effective_def_value(stats, formula_rules)
        }
        StatStageModifierKind::Acc => {
            super::super::helpers::effective_acc_value(stats, formula_rules)
        }
        StatStageModifierKind::Spd => {
            super::super::helpers::effective_spd_value(stats, formula_rules)
        }
    }
}

pub(crate) fn update_player_roster_ui_system(
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    ui_control_side: Res<UiControlSide>,
    combat_query: Query<(&Stats, &Name, &Shield, &ElementAura, &StatusBoard), With<InBattle>>,
    mut text_q: Query<
        (
            &mut Text,
            Option<&TeamMemberButtonText>,
            Option<&TeamMemberAuraText>,
            Option<&TeamMemberNameText>,
            Option<&TeamMemberHpValueText>,
            Option<&TeamMemberShieldValueText>,
            Option<&TeamMemberAtkText>,
            Option<&TeamMemberDefText>,
            Option<&TeamMemberAccText>,
            Option<&TeamMemberSpdText>,
            Option<&PlayerBenchNameText>,
            Option<&PlayerBenchHpValueText>,
            Option<&PlayerBenchShieldValueText>,
        ),
        (Without<ActionPointsText>, Without<PlayerCardNameText>),
    >,
    mut nodes: ParamSet<(
        Query<(&TeamMemberHpBarFill, &mut Node)>,
        Query<(&TeamMemberShieldBarFill, &mut Node)>,
        Query<(
            &TeamMemberButton,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
        Query<(&PlayerBenchHpBarFill, &mut Node)>,
        Query<
            (&PlayerBenchShieldBarTrack, &mut Node, &mut Visibility),
            Without<TeamMemberShieldBarTrack>,
        >,
        Query<(
            &PlayerBenchCard,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        )>,
        Query<
            (&TeamMemberShieldBarTrack, &mut Node, &mut Visibility),
            Without<PlayerBenchShieldBarTrack>,
        >,
    )>,
    theme: Res<UiTheme>,
    formula_rules: Res<BattleFormulaRules>,
) {
    let player_team = player_team.map(|team| team.0.clone());
    let enemy_team = enemy_team.map(|team| team.0.clone());
    let controlled_team = match ui_control_side.0 {
        Side::Player => player_team.clone(),
        Side::Enemy => enemy_team,
    };
    let Some(controlled_team) = controlled_team else {
        return;
    };
    let Some(player_team) = player_team else {
        return;
    };

    let get_controlled_entity = |index: usize| controlled_team.combatants.get(index).copied();

    let is_dead = |entity: Entity| {
        combat_query
            .get(entity)
            .is_ok_and(|(stats, _, _, _, _)| stats.hp <= 0)
    };
    // 待机位展示顺序：存活精灵靠前、阵亡精灵沉底。
    let player_display_order = bench_display_order(&player_team, is_dead);
    // 待机卡片的展示位（meta.index）→ 真实队伍索引 → 实体。
    // 通过这一层映射实现“阵亡沉底”，而不改动队伍真实顺序。
    let get_player_entity = |display_index: usize| {
        player_display_order
            .get(display_index)
            .and_then(|&team_index| player_team.combatants.get(team_index).copied())
    };

    for (
        mut text,
        overlay_name,
        overlay_aura,
        switch_name,
        switch_hp,
        switch_shield,
        switch_atk,
        switch_def,
        switch_acc,
        switch_spd,
        bench_name,
        bench_hp,
        bench_shield,
    ) in &mut text_q
    {
        if let Some(meta) = overlay_name {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((_stats, name, _, aura, _)) = combat_query.get(entity) {
                    let aura_text = super::super::helpers::compact_aura_label(&aura.elements());
                    text.0 = if aura_text.is_empty() {
                        format!("{}键：{}", meta.index + 5, name)
                    } else {
                        format!("{}键：{} {}", meta.index + 5, name, aura_text)
                    };
                } else {
                    text.0 = format!("{}键：队伍{}", meta.index + 5, meta.index + 1);
                }
            } else {
                text.0 = format!("{}键：队伍{}", meta.index + 5, meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = overlay_aura {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((stats, _, _, _, statuses)) = combat_query.get(entity) {
                    text.0 = super::super::helpers::status_stage_label(stats, statuses);
                } else {
                    text.0 = "状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
                }
            } else {
                text.0 = "状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
            }
            continue;
        }

        if let Some(meta) = switch_name {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((_stats, name, _, _, _)) = combat_query.get(entity) {
                    text.0 = name.to_string();
                } else {
                    text.0 = format!("队伍{}", meta.index + 1);
                }
            } else {
                text.0 = format!("队伍{}", meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = switch_hp {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = format!("{}/{}", stats.hp.max(0), stats.max_hp);
                } else {
                    text.0 = "0/0".to_string();
                }
            } else {
                text.0 = "0/0".to_string();
            }
            continue;
        }

        if let Some(meta) = switch_shield {
            if let Some(entity) = get_controlled_entity(meta.index) {
                if let Ok((_, _, shield, _, _)) = combat_query.get(entity) {
                    text.0 = shield_value_text(shield.0);
                } else {
                    text.0 = String::new();
                }
            } else {
                text.0 = String::new();
            }
            continue;
        }

        let switch_stat = switch_atk
            .map(|meta| (meta.index, StatStageModifierKind::Atk))
            .or_else(|| switch_def.map(|meta| (meta.index, StatStageModifierKind::Def)))
            .or_else(|| switch_acc.map(|meta| (meta.index, StatStageModifierKind::Acc)))
            .or_else(|| switch_spd.map(|meta| (meta.index, StatStageModifierKind::Spd)));
        if let Some((index, stat)) = switch_stat {
            if let Some(entity) = get_controlled_entity(index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = effective_stat_value(stats, stat, &formula_rules).to_string();
                } else {
                    text.0 = "0".to_string();
                }
            } else {
                text.0 = "0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_name {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((_stats, name, _, _, _)) = combat_query.get(entity) {
                    text.0 = name.to_string();
                } else {
                    text.0 = format!("队伍{}", meta.index + 1);
                }
            } else {
                text.0 = format!("队伍{}", meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = bench_hp {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = format!("{}/{}", stats.hp.max(0), stats.max_hp);
                } else {
                    text.0 = "0/0".to_string();
                }
            } else {
                text.0 = "0/0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_shield {
            if let Some(entity) = get_player_entity(meta.index) {
                if let Ok((_, _, shield, _, _)) = combat_query.get(entity) {
                    text.0 = shield_value_text(shield.0);
                } else {
                    text.0 = String::new();
                }
            } else {
                text.0 = String::new();
            }
        }
    }

    for (meta, mut node) in &mut nodes.p0() {
        if let Some(entity) = get_controlled_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p1() {
        if let Some(entity) = get_controlled_entity(meta.index) {
            if let Ok((stats, _, shield, _, _)) = combat_query.get(entity) {
                node.width = Val::Percent(shield_pct(stats, shield));
            }
        }
    }

    for (meta, mut node) in &mut nodes.p3() {
        if let Some(entity) = get_player_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node, mut visibility) in &mut nodes.p4() {
        let pct = get_player_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(stats, _, shield, _, _)| shield_pct(stats, shield))
            .unwrap_or(0.0);
        apply_bench_shield_track(&mut node, &mut visibility, pct);
    }

    for (meta, interaction, mut node, mut bg, mut border) in &mut nodes.p2() {
        let exists = get_controlled_entity(meta.index).is_some();
        let active = meta.index == controlled_team.active_index;
        let dead = get_controlled_entity(meta.index).is_some_and(is_dead);
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
        node.min_height = Val::Px(58.0);
        // 阵亡成员统一用冷灰暗底 + 灰描边，替代金色描边以传达“不可用”。
        if dead {
            *bg = BackgroundColor(BENCH_BG_DEAD);
            *border = BorderColor::all(DEAD_MEMBER_BORDER);
        } else {
            *bg = match *interaction {
                Interaction::Hovered => BackgroundColor(Color::srgba(1.0, 0.82, 0.35, 0.08)),
                Interaction::Pressed => BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.18)),
                Interaction::None => BackgroundColor(Color::NONE),
            };
            *border = match *interaction {
                Interaction::Hovered => BorderColor::all(theme.gold_bright),
                Interaction::Pressed => BorderColor::all(theme.gold_dim),
                Interaction::None => BorderColor::all(theme.gold),
            };
        }
    }

    for (meta, interaction, mut node, mut bg) in &mut nodes.p5() {
        let entity = get_player_entity(meta.index);
        let exists = entity.is_some();
        // 待机卡片的展示位已映射到真实队伍索引，据此判断是否为当前上场精灵。
        let team_index = player_display_order.get(meta.index).copied();
        let active = team_index == Some(player_team.active_index);
        let dead = entity.is_some_and(is_dead);
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
        // 无边框：仅用半透明底色区分悬停/按下，去掉原本的“黑方框”。
        // 阵亡位用暗化冷灰底，与存活位的暖底拉开反差。
        *bg = if dead {
            BackgroundColor(BENCH_BG_DEAD)
        } else {
            match *interaction {
                Interaction::Hovered => BackgroundColor(BENCH_BG_HOVER),
                Interaction::Pressed => BackgroundColor(BENCH_BG_PRESSED),
                Interaction::None => BackgroundColor(BENCH_BG),
            }
        };
    }

    for (meta, mut node, mut visibility) in &mut nodes.p6() {
        let pct = get_controlled_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(stats, _, shield, _, _)| shield_pct(stats, shield))
            .unwrap_or(0.0);
        apply_switch_shield_track(&mut node, &mut visibility, pct);
    }
}

pub(crate) fn update_enemy_roster_ui_system(
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Stats, &Name, &Shield, &ElementAura, &StatusBoard), With<InBattle>>,
    mut text_q: Query<
        (
            &mut Text,
            Option<&EnemyTeamMemberButtonText>,
            Option<&EnemyTeamMemberAuraText>,
            Option<&EnemyBenchNameText>,
            Option<&EnemyBenchHpValueText>,
            Option<&EnemyBenchShieldValueText>,
        ),
        (Without<ActionPointsText>, Without<PlayerCardNameText>),
    >,
    mut nodes: ParamSet<(
        Query<(&EnemyTeamMemberHpBarFill, &mut Node)>,
        Query<(&EnemyTeamMemberShieldBarFill, &mut Node)>,
        Query<(
            &EnemyTeamMemberButton,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        )>,
        Query<(&EnemyBenchHpBarFill, &mut Node)>,
        Query<
            (&EnemyBenchShieldBarTrack, &mut Node, &mut Visibility),
            Without<EnemyTeamMemberShieldBarTrack>,
        >,
        Query<(&EnemyBenchCard, &mut Node, &mut BackgroundColor)>,
    )>,
    mut team_member_shield_tracks: Query<
        (&EnemyTeamMemberShieldBarTrack, &mut Visibility),
        Without<EnemyBenchShieldBarTrack>,
    >,
    theme: Res<UiTheme>,
) {
    let Some(enemy_team) = enemy_team else {
        return;
    };

    let is_dead = |entity: Entity| {
        combat_query
            .get(entity)
            .is_ok_and(|(stats, _, _, _, _)| stats.hp <= 0)
    };
    // 待机位展示顺序：存活精灵靠前、阵亡精灵沉底。
    let enemy_display_order = bench_display_order(&enemy_team.0, is_dead);
    // 待机卡片的展示位（meta.index）→ 真实队伍索引 → 实体。
    let get_entity = |display_index: usize| {
        enemy_display_order
            .get(display_index)
            .and_then(|&team_index| enemy_team.0.combatants.get(team_index).copied())
    };

    for (mut text, overlay_name, overlay_aura, bench_name, bench_hp, bench_shield) in &mut text_q {
        if let Some(meta) = overlay_name {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((_stats, name, _, _, _)) = combat_query.get(entity) {
                    text.0 = name.to_string();
                } else {
                    text.0 = format!("队伍{}", meta.index + 1);
                }
            } else {
                text.0 = format!("队伍{}", meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = overlay_aura {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, _, _, aura, statuses)) = combat_query.get(entity) {
                    text.0 = super::super::helpers::aura_status_stage_label(
                        stats,
                        &aura.elements(),
                        statuses,
                    );
                } else {
                    text.0 = "附着: 无 | 状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
                }
            } else {
                text.0 = "附着: 无 | 状态: 无 | 阶段: Atk+0 Def+0 Spd+0 Acc+0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_name {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((_stats, name, _, _, _)) = combat_query.get(entity) {
                    text.0 = name.to_string();
                } else {
                    text.0 = format!("队伍{}", meta.index + 1);
                }
            } else {
                text.0 = format!("队伍{}", meta.index + 1);
            }
            continue;
        }

        if let Some(meta) = bench_hp {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                    text.0 = format!("{}/{}", stats.hp.max(0), stats.max_hp);
                } else {
                    text.0 = "0/0".to_string();
                }
            } else {
                text.0 = "0/0".to_string();
            }
            continue;
        }

        if let Some(meta) = bench_shield {
            if let Some(entity) = get_entity(meta.index) {
                if let Ok((_, _, shield, _, _)) = combat_query.get(entity) {
                    text.0 = shield_value_text(shield.0);
                } else {
                    text.0 = String::new();
                }
            } else {
                text.0 = String::new();
            }
        }
    }

    for (meta, mut node) in &mut nodes.p0() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node) in &mut nodes.p1() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, shield, _, _)) = combat_query.get(entity) {
                node.width = Val::Percent(shield_pct(stats, shield));
            }
        }
    }

    for (meta, mut node) in &mut nodes.p3() {
        if let Some(entity) = get_entity(meta.index) {
            if let Ok((stats, _, _, _, _)) = combat_query.get(entity) {
                let hp_pct = if stats.max_hp > 0 {
                    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                node.width = Val::Percent(hp_pct);
            }
        }
    }

    for (meta, mut node, mut visibility) in &mut nodes.p4() {
        let pct = get_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(stats, _, shield, _, _)| shield_pct(stats, shield))
            .unwrap_or(0.0);
        apply_bench_shield_track(&mut node, &mut visibility, pct);
    }

    for (meta, mut node, mut bg, mut border) in &mut nodes.p2() {
        let exists = get_entity(meta.index).is_some();
        let active = meta.index == enemy_team.0.active_index;
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
        node.min_height = Val::Px(58.0);
        if active {
            *bg = BackgroundColor(theme.button_hover);
            *border = BorderColor::all(theme.button_border_hover);
        } else {
            *bg = BackgroundColor(theme.enemy_card_bg);
            *border = BorderColor::all(theme.enemy_card_border);
        }
    }

    for (meta, mut node, mut bg) in &mut nodes.p5() {
        let entity = get_entity(meta.index);
        let exists = entity.is_some();
        let team_index = enemy_display_order.get(meta.index).copied();
        let active = team_index == Some(enemy_team.0.active_index);
        let dead = entity.is_some_and(is_dead);
        node.display = if !exists || active {
            Display::None
        } else {
            Display::Flex
        };
        // 阵亡位用暗化冷灰底，与存活位的暖底拉开反差。
        *bg = if dead {
            BackgroundColor(BENCH_BG_DEAD)
        } else {
            BackgroundColor(BENCH_BG)
        };
    }

    for (meta, mut vis) in &mut team_member_shield_tracks {
        let has_shield = get_entity(meta.index)
            .and_then(|entity| combat_query.get(entity).ok())
            .map(|(_, _, shield, _, _)| shield.0 > 0)
            .unwrap_or(false);
        *vis = if has_shield {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// 阵亡精灵名字变色系统：移除“已倒下”文字后，用名字颜色替代——
/// 阵亡成员名字压暗成冷灰，存活成员恢复各自阵营强调色。
///
/// 覆盖待机位（玩家/敌方）与换人面板（受控方）的名字文本。
/// 待机位的名字文本在展示位上，需要与待机卡片相同的“存活靠前、阵亡沉底”
/// 展示顺序映射对应到真实队伍成员，否则会对错的人判断生死。
pub(crate) fn update_dead_member_name_color_system(
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    ui_control_side: Res<UiControlSide>,
    combat_query: Query<&Stats, With<InBattle>>,
    theme: Res<UiTheme>,
    mut names: Query<(
        &mut TextColor,
        Option<&PlayerBenchNameText>,
        Option<&EnemyBenchNameText>,
        Option<&TeamMemberNameText>,
    )>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };
    let controlled_team = match ui_control_side.0 {
        Side::Player => &player_team.0,
        Side::Enemy => &enemy_team.0,
    };

    let is_dead = |entity: Entity| combat_query.get(entity).is_ok_and(|stats| stats.hp <= 0);
    // 待机位展示顺序需与待机卡片系统一致，才能对到正确的队伍成员。
    let player_order = bench_display_order(&player_team.0, is_dead);
    let enemy_order = bench_display_order(&enemy_team.0, is_dead);
    // 待机展示位 → 真实队伍索引。
    let bench_team_index =
        |display_index: usize, order: &[usize]| order.get(display_index).copied();

    for (mut color, p_bench, e_bench, switch) in &mut names {
        let dead = if let Some(meta) = p_bench {
            bench_team_index(meta.index, &player_order)
                .and_then(|i| player_team.0.combatants.get(i).copied())
        } else if let Some(meta) = e_bench {
            bench_team_index(meta.index, &enemy_order)
                .and_then(|i| enemy_team.0.combatants.get(i).copied())
        } else if let Some(meta) = switch {
            // 换人面板保持队伍真实顺序，不做展示重排。
            controlled_team.combatants.get(meta.index).copied()
        } else {
            continue;
        }
        .is_some_and(is_dead);
        let alive_color = if e_bench.is_some() {
            theme.accent_enemy
        } else {
            theme.accent_player
        };
        *color = TextColor(if dead { DEAD_NAME_COLOR } else { alive_color });
    }
}
