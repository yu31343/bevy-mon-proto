use bevy::prelude::*;

use super::super::{components::*, resources::UiFontHandle, theme::UiTheme};
use super::roster::bench_display_order;
use crate::{
    battle::{
        BattleEvent, Combatant, ElementAura, EnemyTeam, InBattle, PendingHandDiscard, PlayerTeam,
        RoundOrder, Shield, Side, SkillCount, SkillList, Stats, StatusBoard, Team, TurnCount,
        UiControlSide,
    },
    data::{BattleDbs, BattleFormulaRules, ElementType},
    game_state::BattlePhase,
};

fn element_icon_path(element: ElementType) -> &'static str {
    match element {
        ElementType::Fire => "images/icons/elements/fire.png",
        ElementType::Water => "images/icons/elements/water.png",
        ElementType::Grass => "images/icons/elements/grass.png",
        ElementType::Light => "images/icons/elements/light.png",
        ElementType::Dark => "images/icons/elements/dark.png",
        ElementType::Thunder => "images/icons/elements/thunder.png",
        ElementType::Wind => "images/icons/elements/wind.png",
    }
}

fn portrait_path(element: ElementType) -> &'static str {
    match element {
        ElementType::Fire => "images/icons/profile/ui/fire.png",
        ElementType::Water => "images/icons/profile/ui/water.png",
        ElementType::Grass => "images/icons/profile/ui/grass.png",
        ElementType::Light => "images/icons/profile/ui/light.png",
        ElementType::Dark => "images/icons/profile/ui/dark.png",
        ElementType::Thunder => "images/icons/profile/ui/thunder.png",
        ElementType::Wind => "images/icons/profile/ui/wind.png",
    }
}

fn attachment_icon_path(element: ElementType) -> Option<&'static str> {
    match element {
        ElementType::Fire => Some("images/icons/attachment/fire.png"),
        ElementType::Water => Some("images/icons/attachment/water.png"),
        ElementType::Grass => Some("images/icons/attachment/grass.png"),
        ElementType::Thunder => Some("images/icons/attachment/thunder.png"),
        ElementType::Light | ElementType::Dark | ElementType::Wind => None,
    }
}

type ActiveCombatantRef<'a> = (
    &'a Combatant,
    &'a Stats,
    &'a Name,
    &'a Shield,
    &'a ElementAura,
    &'a StatusBoard,
);

fn status_icon_path(status_id: &str, status_name: &str) -> Option<&'static str> {
    match status_id {
        "seeded" => Some("images/icons/status/绽放.png"),
        "electrocuted" => Some("images/icons/status/导电.png"),
        "armor_break" => Some("images/icons/status/超载.png"),
        _ => match status_name {
            "燃烧" => Some("images/icons/status/燃烧.png"),
            "速度降低" => Some("images/icons/status/速度降低.png"),
            "缠绕" => Some("images/icons/status/缠绕.png"),
            "诅咒" => Some("images/icons/status/诅咒.png"),
            "攻击降低" => Some("images/icons/status/攻击降低.png"),
            "自然治愈" => Some("images/icons/status/自然治愈.png"),
            "闪避" => Some("images/icons/status/闪避.png"),
            _ => None,
        },
    }
}

fn active_combatant_data<'a>(
    team: &Team,
    query: &'a Query<
        (
            &Combatant,
            &Stats,
            &Name,
            &Shield,
            &ElementAura,
            &StatusBoard,
        ),
        With<InBattle>,
    >,
) -> Option<ActiveCombatantRef<'a>> {
    let entity = team.active_combatant()?;
    query.get(entity).ok()
}

fn active_summary(active: Option<ActiveCombatantRef<'_>>) -> String {
    let Some((combatant, _, _, _, _, _)) = active else {
        return "无".to_string();
    };
    super::super::helpers::element_name(combatant.element).to_string()
}

fn stat_value(_stage: i32, current: i32) -> String {
    current.to_string()
}

fn shield_value(value: i32) -> String {
    if value > 0 {
        value.to_string()
    } else {
        String::new()
    }
}

fn stage_modifier_color(stage: i32) -> Color {
    if stage > 0 {
        Color::srgb(0.16, 0.46, 0.95)
    } else {
        Color::srgb(0.86, 0.16, 0.22)
    }
}

fn stat_stage(stats: &Stats, stat: StatStageModifierKind) -> i32 {
    match stat {
        StatStageModifierKind::Atk => stats.atk_stage,
        StatStageModifierKind::Def => stats.def_stage,
        StatStageModifierKind::Acc => stats.acc_stage,
        StatStageModifierKind::Spd => stats.spd_stage,
    }
}

fn stage_for_side(
    player_active: Option<ActiveCombatantRef<'_>>,
    enemy_active: Option<ActiveCombatantRef<'_>>,
    side: Side,
    stat: StatStageModifierKind,
) -> Option<i32> {
    let active = match side {
        Side::Player => player_active,
        Side::Enemy => enemy_active,
    };
    active.map(|(_, stats, _, _, _, _)| stat_stage(stats, stat))
}

fn top_bar_side_label(side: Side) -> &'static str {
    match side {
        Side::Player => "我方",
        Side::Enemy => "敌方",
    }
}

fn current_top_bar_side(
    battle_phase: BattlePhase,
    pending_discard: Option<&PendingHandDiscard>,
) -> Option<Side> {
    match battle_phase {
        BattlePhase::PlayerTurn => Some(Side::Player),
        BattlePhase::EnemyTurn => Some(Side::Enemy),
        BattlePhase::Discard => pending_discard.map(|pending| pending.side),
        _ => None,
    }
}

pub(crate) fn update_phase_text_system(
    battle_phase: Res<State<BattlePhase>>,
    turn_count: Res<TurnCount>,
    round_order: Res<RoundOrder>,
    pending_discard: Option<Res<PendingHandDiscard>>,
    mut text_q: ParamSet<(
        Query<&mut Text, With<BattlePhaseText>>,
        Query<&mut Text, With<BattleTurnOrderText>>,
    )>,
) {
    if let Ok(mut text) = text_q.p0().single_mut() {
        text.0 = if turn_count.0 == 0 {
            "准备中".to_string()
        } else {
            format!("第 {} 回合", turn_count.0)
        };
    }

    if let Ok(mut text) = text_q.p1().single_mut() {
        let current = current_top_bar_side(*battle_phase.get(), pending_discard.as_deref());
        let marker = |side| {
            if current == Some(side) { "●" } else { "○" }
        };
        text.0 = format!(
            "{} {} → {} {}",
            top_bar_side_label(round_order.first),
            marker(round_order.first),
            top_bar_side_label(round_order.second),
            marker(round_order.second)
        );
    }
}

pub(crate) fn update_element_icon_system(
    asset_server: Res<AssetServer>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    ui_control_side: Res<UiControlSide>,
    combat_query: Query<(&Combatant,), With<InBattle>>,
    mut icons: Query<
        (
            &mut ImageNode,
            Option<&PlayerElementIcon>,
            Option<&EnemyElementIcon>,
            Option<&TeamMemberElementIcon>,
        ),
        Or<(
            With<PlayerElementIcon>,
            With<EnemyElementIcon>,
            With<TeamMemberElementIcon>,
        )>,
    >,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_element = player_team
        .0
        .active_combatant()
        .and_then(|entity| combat_query.get(entity).ok())
        .map(|(combatant,)| combatant.element);
    let enemy_element = enemy_team
        .0
        .active_combatant()
        .and_then(|entity| combat_query.get(entity).ok())
        .map(|(combatant,)| combatant.element);
    let controlled_team = match ui_control_side.0 {
        Side::Player => &player_team.0,
        Side::Enemy => &enemy_team.0,
    };

    for (mut image, is_player, is_enemy, switch_icon) in &mut icons {
        let element = if is_player.is_some() {
            player_element
        } else if is_enemy.is_some() {
            enemy_element
        } else if let Some(switch_icon) = switch_icon {
            controlled_team
                .combatants
                .get(switch_icon.index)
                .and_then(|entity| combat_query.get(*entity).ok())
                .map(|(combatant,)| combatant.element)
        } else {
            None
        };

        if let Some(element) = element {
            image.image = asset_server.load(element_icon_path(element));
            image.color = Color::WHITE;
        } else {
            image.color = Color::NONE;
        }
    }
}

/// 刷新精灵头像（上场大头像 + 待机位小头像，敌我两侧）。
///
/// 头像按元素取图（`images/icons/profile/ui/{element}.png`），逻辑与
/// [`update_element_icon_system`] 一致：上场取 `active_combatant()`，
/// 待机按槽位 `combatants[index]`。无精灵（空位/查询失败）则隐藏头像。
/// 敌方水平翻转在生成时静态设置，本系统不触碰 `flip_x`。
pub(crate) fn update_portrait_images_system(
    asset_server: Res<AssetServer>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Combatant, &Stats), With<InBattle>>,
    mut portraits: Query<
        (
            &mut ImageNode,
            Option<&PlayerPortraitImage>,
            Option<&EnemyPortraitImage>,
            Option<&PlayerBenchPortrait>,
            Option<&EnemyBenchPortrait>,
            Option<&TeamMemberPortrait>,
        ),
        Or<(
            With<PlayerPortraitImage>,
            With<EnemyPortraitImage>,
            With<PlayerBenchPortrait>,
            With<EnemyBenchPortrait>,
            With<TeamMemberPortrait>,
        )>,
    >,
    ui_control_side: Res<UiControlSide>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };
    let controlled_team = match ui_control_side.0 {
        Side::Player => &player_team.0,
        Side::Enemy => &enemy_team.0,
    };

    let is_dead = |entity: Entity| {
        combat_query
            .get(entity)
            .is_ok_and(|(_, stats)| stats.hp <= 0)
    };
    // 待机头像需与待机卡片使用相同的“存活靠前、阵亡沉底”展示顺序，
    // 才能对应到正确的队伍成员，并正确判断阵亡头像变灰。
    let player_order = bench_display_order(&player_team.0, is_dead);
    let enemy_order = bench_display_order(&enemy_team.0, is_dead);
    let bench_entity = |display_index: usize, order: &[usize], team: &Team| {
        order
            .get(display_index)
            .and_then(|&i| team.combatants.get(i).copied())
    };

    for (mut image, p_active, e_active, p_bench, e_bench, switch_portrait) in &mut portraits {
        let entity = if p_active.is_some() {
            player_team.0.active_combatant()
        } else if e_active.is_some() {
            enemy_team.0.active_combatant()
        } else if let Some(bench) = p_bench {
            bench_entity(bench.index, &player_order, &player_team.0)
        } else if let Some(bench) = e_bench {
            bench_entity(bench.index, &enemy_order, &enemy_team.0)
        } else if let Some(switch_portrait) = switch_portrait {
            controlled_team
                .combatants
                .get(switch_portrait.index)
                .copied()
        } else {
            None
        };

        let combat = entity.and_then(|entity| combat_query.get(entity).ok());

        if let Some((combatant, stats)) = combat {
            image.image = asset_server.load(portrait_path(combatant.element));
            // 阵亡精灵头像整体压暗成冷灰，存活保持原色，替代“已倒下”文字。
            image.color = if stats.hp <= 0 {
                DEAD_PORTRAIT_TINT
            } else {
                Color::WHITE
            };
        } else {
            image.color = Color::NONE;
        }
    }
}

/// 阵亡精灵头像的冷灰压暗色调，与存活头像的原色形成明显反差。
const DEAD_PORTRAIT_TINT: Color = Color::srgba(0.42, 0.40, 0.42, 1.0);

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_active_panel_text_system(
    mut text_sets: ParamSet<(
        Query<
            (
                &mut Text,
                Option<&PlayerStatsText>,
                Option<&EnemyStatsText>,
                Option<&PlayerNameText>,
                Option<&EnemyNameText>,
            ),
            Without<BattlePhaseText>,
        >,
        Query<
            (
                &mut Text,
                Option<&PlayerHpValueText>,
                Option<&PlayerHpStatText>,
                Option<&EnemyHpValueText>,
                Option<&EnemyHpStatText>,
                Option<&PlayerShieldValueText>,
                Option<&EnemyShieldValueText>,
                Option<&PlayerAtkText>,
                Option<&EnemyAtkText>,
                Option<&PlayerDefText>,
                Option<&EnemyDefText>,
                Option<&PlayerAccText>,
                Option<&EnemyAccText>,
                Option<&PlayerSpdText>,
                Option<&EnemySpdText>,
            ),
            Without<BattlePhaseText>,
        >,
    )>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<
        (
            &Combatant,
            &Stats,
            &Name,
            &Shield,
            &ElementAura,
            &StatusBoard,
        ),
        With<InBattle>,
    >,
    formula_rules: Res<BattleFormulaRules>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_active = active_combatant_data(&player_team.0, &combat_query);
    let enemy_active = active_combatant_data(&enemy_team.0, &combat_query);
    let player_summary = active_summary(player_active);
    let enemy_summary = active_summary(enemy_active);

    for (mut text, is_player_summary, is_enemy_summary, is_player_name, is_enemy_name) in
        &mut text_sets.p0()
    {
        if is_player_name.is_some() {
            text.0 = if let Some((_, _, name, _, _, _)) = player_active {
                name.to_string()
            } else {
                "无在场精灵".to_string()
            };
            continue;
        }
        if is_enemy_name.is_some() {
            text.0 = if let Some((_, _, name, _, _, _)) = enemy_active {
                name.to_string()
            } else {
                "无在场精灵".to_string()
            };
            continue;
        }
        if is_player_summary.is_some() {
            text.0 = player_summary.clone();
            continue;
        }
        if is_enemy_summary.is_some() {
            text.0 = enemy_summary.clone();
        }
    }

    for (
        mut text,
        is_player_hp,
        is_player_hp_stat,
        is_enemy_hp,
        is_enemy_hp_stat,
        is_player_shield,
        is_enemy_shield,
        is_player_atk,
        is_enemy_atk,
        is_player_def,
        is_enemy_def,
        is_player_acc,
        is_enemy_acc,
        is_player_spd,
        is_enemy_spd,
    ) in &mut text_sets.p1()
    {
        if is_player_hp.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                format!("{}/{}", stats.hp.max(0), stats.max_hp)
            } else {
                "0/0".to_string()
            };
            continue;
        }
        if is_player_hp_stat.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stats.max_hp.to_string()
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_hp.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                format!("{}/{}", stats.hp.max(0), stats.max_hp)
            } else {
                "0/0".to_string()
            };
            continue;
        }
        if is_enemy_hp_stat.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stats.max_hp.to_string()
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_shield.is_some() {
            text.0 = if let Some((_, _, _, shield, _, _)) = player_active {
                shield_value(shield.0)
            } else {
                String::new()
            };
            continue;
        }
        if is_enemy_shield.is_some() {
            text.0 = if let Some((_, _, _, shield, _, _)) = enemy_active {
                shield_value(shield.0)
            } else {
                String::new()
            };
            continue;
        }
        if is_player_atk.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.atk_stage,
                    super::super::helpers::effective_atk_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_atk.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.atk_stage,
                    super::super::helpers::effective_atk_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_def.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.def_stage,
                    super::super::helpers::effective_def_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_def.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.def_stage,
                    super::super::helpers::effective_def_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_acc.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.acc_stage,
                    super::super::helpers::effective_acc_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_acc.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.acc_stage,
                    super::super::helpers::effective_acc_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_player_spd.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = player_active {
                stat_value(
                    stats.spd_stage,
                    super::super::helpers::effective_spd_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
            continue;
        }
        if is_enemy_spd.is_some() {
            text.0 = if let Some((_, stats, _, _, _, _)) = enemy_active {
                stat_value(
                    stats.spd_stage,
                    super::super::helpers::effective_spd_value(stats, &formula_rules),
                )
            } else {
                "0".to_string()
            };
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_active_panel_tokens_system(
    mut commands: Commands,
    mut line_queries: ParamSet<(
        Query<
            (
                Entity,
                Option<&Children>,
                Option<&PlayerAuraLine>,
                Option<&EnemyAuraLine>,
            ),
            Or<(With<PlayerAuraLine>, With<EnemyAuraLine>)>,
        >,
        Query<
            (
                Entity,
                Option<&Children>,
                Option<&PlayerStatusLine>,
                Option<&EnemyStatusLine>,
            ),
            Or<(With<PlayerStatusLine>, With<EnemyStatusLine>)>,
        >,
        Query<(Entity, Option<&Children>, &PlayerBenchAuraLine)>,
        Query<(Entity, Option<&Children>, &EnemyBenchAuraLine)>,
        Query<(Entity, Option<&Children>, &TeamMemberAuraLine)>,
        Query<(Entity, Option<&Children>, &PlayerBenchStatusLine)>,
        Query<(Entity, Option<&Children>, &EnemyBenchStatusLine)>,
        Query<(Entity, Option<&Children>, &TeamMemberStatusLine)>,
    )>,
    mut stage_queries: ParamSet<(
        Query<(&mut Node, &mut BackgroundColor, &StatStageModifierBadge)>,
        Query<(&mut Text, &StatStageModifierText)>,
        Query<(
            &mut Node,
            &mut BackgroundColor,
            &TeamMemberStatStageModifierBadge,
        )>,
        Query<(&mut Text, &TeamMemberStatStageModifierText)>,
    )>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    ui_control_side: Res<UiControlSide>,
    combat_query: Query<
        (
            &Combatant,
            &Stats,
            &Name,
            &Shield,
            &ElementAura,
            &StatusBoard,
        ),
        With<InBattle>,
    >,
    asset_server: Res<AssetServer>,
    theme: Res<UiTheme>,
    ui_font: Option<Res<UiFontHandle>>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_active = active_combatant_data(&player_team.0, &combat_query);
    let enemy_active = active_combatant_data(&enemy_team.0, &combat_query);
    let controlled_team = match ui_control_side.0 {
        Side::Player => &player_team.0,
        Side::Enemy => &enemy_team.0,
    };
    let info_font = super::super::helpers::make_text_font(13.0, ui_font.as_deref());
    let aura_icon_items =
        |target: Option<Entity>| -> Vec<super::super::helpers::DebugTokenContent> {
            let Some(entity) = target else {
                return Vec::new();
            };
            let Ok((_, _, _, _, aura, _)) = combat_query.get(entity) else {
                return Vec::new();
            };
            aura.elements()
                .iter()
                .filter_map(|element| attachment_icon_path(*element))
                .map(super::super::helpers::DebugTokenContent::Image)
                .collect()
        };

    for (entity, children, is_player_aura, is_enemy_aura) in &line_queries.p0() {
        let target = if is_player_aura.is_some() {
            player_team.0.active_combatant()
        } else if is_enemy_aura.is_some() {
            enemy_team.0.active_combatant()
        } else {
            None
        };
        let items = aura_icon_items(target);
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            28.0,
            DebugAuraToken,
        );
    }

    for (entity, children, line) in &line_queries.p2() {
        let items = aura_icon_items(player_team.0.combatants.get(line.index).copied());
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            24.0,
            DebugAuraToken,
        );
    }

    for (entity, children, line) in &line_queries.p3() {
        let items = aura_icon_items(enemy_team.0.combatants.get(line.index).copied());
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            24.0,
            DebugAuraToken,
        );
    }

    for (entity, children, line) in &line_queries.p4() {
        let items = aura_icon_items(controlled_team.combatants.get(line.index).copied());
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            28.0,
            DebugAuraToken,
        );
    }

    for (_, mut bg, marker) in &mut stage_queries.p0() {
        let stage =
            stage_for_side(player_active, enemy_active, marker.side, marker.stat).unwrap_or(0);
        *bg = if stage == 0 {
            BackgroundColor(Color::NONE)
        } else {
            BackgroundColor(stage_modifier_color(stage))
        };
    }

    for (mut text, marker) in &mut stage_queries.p1() {
        let stage =
            stage_for_side(player_active, enemy_active, marker.side, marker.stat).unwrap_or(0);
        text.0 = if stage == 0 {
            String::new()
        } else {
            format!("{stage:+}")
        };
    }

    for (mut node, mut bg, marker) in &mut stage_queries.p2() {
        let stage = controlled_team
            .combatants
            .get(marker.index)
            .and_then(|entity| combat_query.get(*entity).ok())
            .map(|(_, stats, _, _, _, _)| stat_stage(stats, marker.stat))
            .unwrap_or(0);
        node.display = if stage == 0 {
            Display::None
        } else {
            Display::Flex
        };
        *bg = if stage == 0 {
            BackgroundColor(Color::NONE)
        } else {
            BackgroundColor(stage_modifier_color(stage))
        };
    }

    for (mut text, marker) in &mut stage_queries.p3() {
        let stage = controlled_team
            .combatants
            .get(marker.index)
            .and_then(|entity| combat_query.get(*entity).ok())
            .map(|(_, stats, _, _, _, _)| stat_stage(stats, marker.stat))
            .unwrap_or(0);
        text.0 = if stage == 0 {
            String::new()
        } else {
            format!("{stage:+}")
        };
    }

    for (entity, children, is_player_status, is_enemy_status) in &line_queries.p1() {
        let active = if is_player_status.is_some() {
            player_active
        } else if is_enemy_status.is_some() {
            enemy_active
        } else {
            None
        };
        let items = if let Some((_, _, _, _, _, statuses)) = active {
            statuses
                .entries
                .iter()
                .filter(|entry| {
                    super::super::helpers::is_status_line_visible(&entry.id, entry.category)
                })
                .map(|entry| {
                    if let Some(path) = status_icon_path(&entry.id, &entry.name) {
                        super::super::helpers::DebugTokenContent::Image(path)
                    } else {
                        super::super::helpers::DebugTokenContent::Text(
                            entry.name.clone(),
                            super::super::helpers::status_color(entry, &theme),
                        )
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            30.0,
            DebugStatusToken,
        );
    }

    // 待机位状态图标：与主面板状态行使用同一可见性规则与图标，无状态时不显示。
    let bench_status_items =
        |target: Option<Entity>| -> Vec<super::super::helpers::DebugTokenContent> {
            let Some(entity) = target else {
                return Vec::new();
            };
            let Ok((_, _, _, _, _, statuses)) = combat_query.get(entity) else {
                return Vec::new();
            };
            statuses
                .entries
                .iter()
                .filter(|entry| {
                    super::super::helpers::is_status_line_visible(&entry.id, entry.category)
                })
                .map(|entry| {
                    if let Some(path) = status_icon_path(&entry.id, &entry.name) {
                        super::super::helpers::DebugTokenContent::Image(path)
                    } else {
                        super::super::helpers::DebugTokenContent::Text(
                            entry.name.clone(),
                            super::super::helpers::status_color(entry, &theme),
                        )
                    }
                })
                .collect()
        };

    for (entity, children, line) in &line_queries.p5() {
        let items = bench_status_items(player_team.0.combatants.get(line.index).copied());
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            28.0,
            DebugStatusToken,
        );
    }

    for (entity, children, line) in &line_queries.p6() {
        let items = bench_status_items(enemy_team.0.combatants.get(line.index).copied());
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            28.0,
            DebugStatusToken,
        );
    }

    for (entity, children, line) in &line_queries.p7() {
        let items = bench_status_items(controlled_team.combatants.get(line.index).copied());
        super::super::helpers::replace_debug_tokens_with_images(
            &mut commands,
            entity,
            children,
            &asset_server,
            &info_font,
            &items,
            30.0,
            DebugStatusToken,
        );
    }
}

pub(crate) fn update_skill_text_system(
    battle_phase: Res<State<BattlePhase>>,
    ui_control_side: Res<crate::battle::UiControlSide>,
    theme: Res<UiTheme>,
    mut text_q: Query<
        (
            &mut Text,
            Option<&mut TextColor>,
            Option<&mut TextShadow>,
            Option<&TurnBannerText>,
            Option<&SkillButtonText>,
            Option<&SkillButtonMetaText>,
            Option<&SkillButtonIconText>,
            Option<&SkillButtonCostText>,
            Option<&EnemySkillText>,
            Option<&EnemySkillMetaText>,
            Option<&EnemySkillIconText>,
        ),
        (
            Without<ActionPointsText>,
            Without<ResultText>,
            Without<BattleHintText>,
            Without<TeamMemberButtonText>,
            Without<TeamMemberAuraText>,
            Without<PlayerCardHotkeyText>,
            Without<PlayerCardNameText>,
            Without<PlayerCardCostText>,
            Without<PlayerCardDescText>,
            Without<BattleActionText>,
        ),
    >,
    mut type_chip_q: Query<(&SkillTileTypeChip, &mut BackgroundColor)>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    skill_query: Query<(&SkillList, &SkillCount), With<InBattle>>,
    skill_db: Res<BattleDbs>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let mut player_skills = None;
    if let Some(entity) = player_team.0.active_combatant() {
        if let Ok((skills, count)) = skill_query.get(entity) {
            player_skills = Some((skills.0, count.0));
        }
    }

    let mut enemy_skills = None;
    if let Some(entity) = enemy_team.0.active_combatant() {
        if let Ok((skills, count)) = skill_query.get(entity) {
            enemy_skills = Some((skills.0, count.0));
        }
    }

    for (
        mut text,
        text_color,
        text_shadow,
        is_turn_banner,
        skill_button_text,
        skill_button_meta_text,
        skill_icon_text,
        skill_cost_text,
        enemy_skill_text,
        enemy_skill_meta,
        enemy_skill_icon,
    ) in &mut text_q
    {
        if is_turn_banner.is_some() {
            let (banner_text, outline_color) = match *battle_phase.get() {
                BattlePhase::PlayerTurn => ("你的回合", Color::srgba(0.05, 0.24, 1.0, 0.95)),
                BattlePhase::EnemyTurn => ("敌方回合", Color::srgba(1.0, 0.05, 0.04, 0.95)),
                BattlePhase::Discard => ("弃牌阶段", Color::srgba(0.05, 0.24, 1.0, 0.95)),
                _ => ("", Color::srgba(0.0, 0.0, 0.0, 0.0)),
            };
            text.0 = banner_text.to_string();
            if let Some(mut text_color) = text_color {
                text_color.0 = Color::WHITE;
            }
            if let Some(mut text_shadow) = text_shadow {
                text_shadow.offset = Vec2::new(3.0, 3.0);
                text_shadow.color = outline_color;
            }
            continue;
        }
        let control_skills = match ui_control_side.0 {
            crate::battle::Side::Player => player_skills,
            crate::battle::Side::Enemy => enemy_skills,
        };
        if let (Some(button), Some((skills, count))) = (skill_button_text, control_skills) {
            if button.index >= count {
                text.0 = format!("{}号: 未配置", button.index + 1);
            } else {
                let skill_id = skills[button.index];
                text.0 = format!(
                    "{}号: {}",
                    button.index + 1,
                    super::super::helpers::skill_name(skill_id, &skill_db)
                );
            }
            continue;
        }
        if let (Some(meta), Some((skills, count))) = (skill_button_meta_text, control_skills) {
            if meta.index >= count {
                text.0 = "类型：--".to_string();
            } else {
                let skill_id = skills[meta.index];
                text.0 = format!(
                    "{}\n{}",
                    super::super::helpers::skill_meta(skill_id, &skill_db),
                    super::super::helpers::skill_summary(skill_id, &skill_db)
                );
            }
            continue;
        }
        if let (Some(cost), Some((skills, count))) = (skill_cost_text, control_skills) {
            text.0 = if cost.index >= count {
                "—".to_string()
            } else {
                super::super::helpers::monster_skill_ap_cost_ui(skills[cost.index], &skill_db)
                    .to_string()
            };
            continue;
        }
        if let (Some(icon), Some((_skills, count))) = (skill_icon_text, control_skills) {
            text.0 = if icon.index >= count {
                "-".to_string()
            } else {
                (icon.index + 1).to_string()
            };
            continue;
        }
        if let (Some(button), Some((skills, count))) = (enemy_skill_text, enemy_skills) {
            if button.index >= count {
                text.0 = format!("{}号: 未配置", button.index + 1);
            } else {
                let skill_id = skills[button.index];
                text.0 = format!(
                    "{}号: {}",
                    button.index + 1,
                    super::super::helpers::skill_name(skill_id, &skill_db)
                );
            }
            continue;
        }
        if let (Some(meta), Some((skills, count))) = (enemy_skill_meta, enemy_skills) {
            if meta.index >= count {
                text.0 = "类型：--".to_string();
            } else {
                let skill_id = skills[meta.index];
                text.0 = super::super::helpers::skill_meta(skill_id, &skill_db);
            }
            continue;
        }
        if let (Some(icon), Some((_skills, count))) = (enemy_skill_icon, enemy_skills) {
            text.0 = if icon.index >= count {
                "-".to_string()
            } else {
                (icon.index + 1).to_string()
            };
        }
    }

    // 技能格元素类型色片：按 control 侧技能的元素着色（未配置/无元素用描金暗）。
    let chip_skills = match ui_control_side.0 {
        crate::battle::Side::Player => player_skills,
        crate::battle::Side::Enemy => enemy_skills,
    };
    for (chip, mut bg) in &mut type_chip_q {
        let color = match chip_skills {
            Some((skills, count)) if chip.index < count => skill_db
                .skills
                .get(&skills[chip.index])
                .and_then(|skill| skill.element)
                .map(|element| super::super::helpers::element_color(element, &theme))
                .unwrap_or(theme.gold_dim),
            _ => theme.gold_dim,
        };
        *bg = BackgroundColor(color);
    }
}

pub(crate) fn update_action_points_text_system(
    action_points: Res<crate::battle::ActionPoints>,
    mut text_q: Query<&mut Text, With<ActionPointsText>>,
) {
    if let Ok(mut text) = text_q.single_mut() {
        text.0 = format!(
            "AP：Player {} / Enemy {}",
            action_points.player, action_points.enemy
        );
    }
}

/// 用 AP 宝石 pip + 数字直观展示双方行动点（七圣召唤风）。
/// 按真实 Side 读取，PVP 镜像下两侧各自正确。
pub(crate) fn update_ap_gems_system(
    action_points: Res<crate::battle::ActionPoints>,
    theme: Res<UiTheme>,
    mut pip_q: Query<&mut BackgroundColor, (With<ApGemPip>, Without<ApGemPipFill>)>,
    mut fill_q: Query<(&ApGemPipFill, &mut Node, &mut BackgroundColor), Without<ApGemPip>>,
    mut count_q: Query<(&ApGemCountText, &mut Text)>,
) {
    let ap_for = |side: Side| match side {
        Side::Player => action_points.player,
        Side::Enemy => action_points.enemy,
    };
    for mut bg in &mut pip_q {
        *bg = BackgroundColor(theme.ap_gem_empty);
    }
    for (fill, mut node, mut bg) in &mut fill_q {
        let filled_units = (ap_for(fill.side).max(0) - fill.index as i32 * 2).clamp(0, 2);
        node.width = Val::Percent(filled_units as f32 * 50.0);
        *bg = BackgroundColor(theme.ap_gem_full);
    }
    for (count, mut text) in &mut count_q {
        text.0 = ap_for(count.side).to_string();
    }
}

const BATTLE_ACTION_TEXT_VISIBLE_SECONDS: f32 = 2.0;

pub(crate) fn update_battle_action_text_system(
    time: Res<Time>,
    mut events: MessageReader<BattleEvent>,
    mut text_q: Query<(&mut Text, &mut Visibility, &mut BattleActionText)>,
) {
    let Ok((mut text, mut visibility, mut action_text)) = text_q.single_mut() else {
        return;
    };

    for event in events.read() {
        let line = match event {
            BattleEvent::SkillUsed {
                side, skill_name, ..
            } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("{}发动{}", owner, skill_name)
            }
            BattleEvent::CardUsed { side, card_name } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("{}使用技能牌{}", owner, card_name)
            }
            BattleEvent::CardDiscarded { side, card_name } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("{}弃置{}", owner, card_name)
            }
            BattleEvent::Switched { side, name } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("{}换上{}", owner, name)
            }
            BattleEvent::CombatantFainted { side, name, .. } => {
                let owner = if *side == crate::battle::Side::Player {
                    "我方"
                } else {
                    "对方"
                };
                format!("{}{}倒下", owner, name)
            }
            _ => continue,
        };
        text.0 = line;
        action_text.remaining = BATTLE_ACTION_TEXT_VISIBLE_SECONDS;
        *visibility = Visibility::Visible;
    }

    if action_text.remaining > 0.0 {
        action_text.remaining = (action_text.remaining - time.delta_secs()).max(0.0);
        if action_text.remaining == 0.0 {
            text.0.clear();
            *visibility = Visibility::Hidden;
        }
    }
}

pub(crate) fn update_result_ui_system(
    mut result_text_q: Query<&mut Text, With<ResultText>>,
    battle_result: Res<crate::battle::BattleResult>,
) {
    if let Ok(mut result_text) = result_text_q.single_mut() {
        if battle_result.message.is_empty() {
            result_text.0.clear();
            return;
        }

        let mut lines = vec![battle_result.message.clone()];
        if let Some(status) = &battle_result.export_status {
            lines.push(status.clone());
        }
        lines.push("按 L 导出 replay/action log。".to_string());
        result_text.0 = lines.join("\n");
    }
}
