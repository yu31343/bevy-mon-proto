use bevy::prelude::*;
use std::fs;

use crate::{
    battle::{
        BattleLog, BattleResult, Combatant, EnemyTeam, InBattle, PlayerTeam, Shield, SkillList, Stats, Team,
        TurnContext,
    },
    data::{ElementType, SkillDb, SkillEffect, SkillId},
    game_state::{BattlePhase, GameState},
};

/// 玩家属性文本标记。
#[derive(Component)]
struct PlayerStatsText;
/// 敌方属性文本标记。
#[derive(Component)]
struct EnemyStatsText;
/// 日志文本标记。
#[derive(Component)]
struct LogText;
/// 结算文本标记。
#[derive(Component)]
struct ResultText;
/// 战斗阶段文本标记。
#[derive(Component)]
struct BattlePhaseText;
/// 技能按钮标记（保存按钮槽位索引）。
#[derive(Component)]
struct SkillButton {
    index: usize,
}
/// 技能按钮内文本标记（保存按钮槽位索引）。
#[derive(Component)]
struct SkillButtonText {
    index: usize,
}
/// 技能按钮副信息文本标记（保存按钮槽位索引）。
#[derive(Component)]
struct SkillButtonMetaText {
    index: usize,
}
/// 技能按钮图标占位文本标记（保存按钮槽位索引）。
#[derive(Component)]
struct SkillButtonIconText {
    index: usize,
}
/// 玩家血条填充条。
#[derive(Component)]
struct PlayerHpBarFill;
/// 敌方血条填充条。
#[derive(Component)]
struct EnemyHpBarFill;

/// UI 插件：负责渲染和更新战斗界面。
pub struct UiPlugin;

#[derive(Resource, Clone)]
struct UiFontHandle(Handle<Font>);

#[derive(Resource, Clone)]
struct UiTheme {
    bg_main: Color,
    panel: Color,
    button_idle: Color,
    button_hover: Color,
    button_pressed: Color,
    hp_track: Color,
    hp_fill_player: Color,
    hp_fill_enemy: Color,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            bg_main: Color::srgb(0.07, 0.10, 0.14),
            panel: Color::srgb(0.12, 0.16, 0.22),
            button_idle: Color::srgb(0.18, 0.26, 0.34),
            button_hover: Color::srgb(0.23, 0.33, 0.43),
            button_pressed: Color::srgb(0.12, 0.22, 0.30),
            hp_track: Color::srgb(0.22, 0.22, 0.24),
            hp_fill_player: Color::srgb(0.17, 0.73, 0.45),
            hp_fill_enemy: Color::srgb(0.89, 0.31, 0.33),
        }
    }
}

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTheme>()
            .add_systems(Startup, (spawn_camera, load_cjk_font_system, setup_ui_system).chain())
            .add_systems(
                Update,
                (
                    button_select_skill_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerCommand))),
                    button_visual_state_system.run_if(in_state(GameState::Battle)),
                    update_battle_ui_system.run_if(in_state(GameState::Battle)),
                    update_result_ui_system.run_if(in_state(GameState::Result)),
                ),
            );
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// 尝试加载支持中文的系统字体，优先使用本机常见字体。
fn load_cjk_font_system(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let candidates = [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/msyh.ttf",
        "C:/Windows/Fonts/simhei.ttf",
        "C:/Windows/Fonts/simsun.ttc",
        "C:/Windows/Fonts/simkai.ttf",
    ];

    for path in candidates {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Ok(font) = Font::try_from_bytes(bytes) else {
            continue;
        };
        let handle = fonts.add(font);
        commands.insert_resource(UiFontHandle(handle));
        return;
    }
}

/// 创建战斗 UI 节点树（属性、血条、技能栏、日志、结算文本）。
fn setup_ui_system(mut commands: Commands, theme: Res<UiTheme>, ui_font: Option<Res<UiFontHandle>>) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(theme.bg_main),
        ))
        .with_children(|root| {
            let title_font = make_text_font(24.0, ui_font.as_deref());
            let body_font = make_text_font(19.0, ui_font.as_deref());
            let log_font = make_text_font(17.0, ui_font.as_deref());
            let result_font = make_text_font(28.0, ui_font.as_deref());
            let icon_font = make_text_font(16.0, ui_font.as_deref());
            let meta_font = make_text_font(14.0, ui_font.as_deref());

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.20, 0.35, 0.46, 0.50)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Text::new("战斗阶段：准备中"),
                    body_font.clone(),
                    TextColor(Color::srgb(0.95, 0.98, 1.0)),
                    BattlePhaseText,
                ));
                bar.spawn((
                    Text::new("操作提示：按 1-4 选择技能，按 Q 切换成员，按 R 重新开始"),
                    meta_font.clone(),
                    TextColor(Color::srgb(0.80, 0.90, 0.95)),
                ));
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(theme.panel),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("玩家：..."),
                    title_font.clone(),
                    TextColor(Color::WHITE),
                    PlayerStatsText,
                ));
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(14.0),
                            ..default()
                        },
                        BackgroundColor(theme.hp_track),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(theme.hp_fill_player),
                            PlayerHpBarFill,
                        ));
                    });

                panel.spawn((
                    Text::new("敌方：..."),
                    title_font,
                    TextColor(Color::WHITE),
                    EnemyStatsText,
                ));
                panel
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(14.0),
                            ..default()
                        },
                        BackgroundColor(theme.hp_track),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(theme.hp_fill_enemy),
                            EnemyHpBarFill,
                        ));
                    });
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(120.0),
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(8.0),
                    align_items: AlignItems::Stretch,
                    ..default()
                },
            ))
            .with_children(|row| {
                for idx in 0..4 {
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Percent(49.0),
                            min_height: Val::Px(64.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(theme.button_idle),
                        SkillButton { index: idx },
                    ))
                    .with_children(|button| {
                        button
                            .spawn((
                                Node {
                                    width: Val::Px(24.0),
                                    height: Val::Px(24.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
                            ))
                            .with_children(|icon_box| {
                                icon_box.spawn((
                                    Text::new("?"),
                                    icon_font.clone(),
                                    TextColor(Color::WHITE),
                                    SkillButtonIconText { index: idx },
                                ));
                            });

                        button
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(2.0),
                                    ..default()
                                },
                            ))
                            .with_children(|column| {
                                column.spawn((
                                    Text::new(format!("技能 {}", idx + 1)),
                                    body_font.clone(),
                                    TextColor(Color::WHITE),
                                    SkillButtonText { index: idx },
                                ));
                                column.spawn((
                                    Text::new("类型：--"),
                                    meta_font.clone(),
                                    TextColor(Color::srgb(0.70, 0.82, 0.92)),
                                    SkillButtonMetaText { index: idx },
                                ));
                            });
                    });
                }
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(140.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.07, 0.11, 0.80)),
            ))
            .with_children(|log_panel| {
                log_panel.spawn((
                    Text::new("战斗日志"),
                    body_font,
                    TextColor(Color::srgb(0.93, 0.96, 0.98)),
                ));
                log_panel.spawn((
                    Text::new(""),
                    log_font,
                    TextColor(Color::srgb(0.82, 0.87, 0.90)),
                    LogText,
                ));
            });

            root.spawn((
                Text::new(""),
                result_font,
                TextColor(Color::srgb(1.0, 0.82, 0.35)),
                ResultText,
            ));
        });
}

/// 处理技能按钮点击，行为与键盘选招保持一致。
fn button_select_skill_system(
    mut interaction_query: Query<(&Interaction, &SkillButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    player_team: Option<Res<PlayerTeam>>,
    query: Query<&SkillList, With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    if let Some(player_team) = player_team {
        if let Some(active_entity) = player_team.0.active_combatant() {
            let Ok(skills) = query.get(active_entity) else {
                return;
            };
            let skills = skills.0;

            for (interaction, button) in &mut interaction_query {
                if *interaction == Interaction::Pressed {
                    turn_ctx.player_action = Some(crate::battle::TurnAction::Skill(skills[button.index]));
                    next_phase.set(BattlePhase::EnemyCommand);
                }
            }
        }
    }
}

/// 根据交互状态更新按钮色，先用统一主题，后期可切换贴图方案。
fn button_visual_state_system(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<SkillButton>)>,
    theme: Res<UiTheme>,
) {
    for (interaction, mut bg) in &mut buttons {
        *bg = match *interaction {
            Interaction::Pressed => BackgroundColor(theme.button_pressed),
            Interaction::Hovered => BackgroundColor(theme.button_hover),
            Interaction::None => BackgroundColor(theme.button_idle),
        };
    }
}

/// 将队伍编成多行面板文案：存活汇总、每位成员场上/替补、生命、护盾、倒下。
fn format_team_roster(
    header: &str,
    team: &Team,
    query: &Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>,
) -> String {
    let total = team.combatants.len();
    let alive = team
        .combatants
        .iter()
        .filter(|&&e| query.get(e).map(|(_, stats, _, _)| stats.hp > 0).unwrap_or(false))
        .count();

    let mut lines = vec![format!("{}（存活 {}/{}）", header, alive, total)];

    for (i, &entity) in team.combatants.iter().enumerate() {
        let Ok((combatant, stats, name, shield)) = query.get(entity) else {
            continue;
        };
        let slot = if i == team.active_index { "[场上]" } else { "[替补]" };
        let shield_str = if shield.0 > 0 {
            format!(" 护盾: {}", shield.0)
        } else {
            String::new()
        };
        let faint = if stats.hp <= 0 { " 倒下" } else { "" };
        lines.push(format!(
            "{} [{}] {} [{}] 生命: {}/{}{}{}",
            slot,
            combatant.side,
            name,
            element_name(combatant.element),
            stats.hp,
            stats.max_hp,
            shield_str,
            faint,
        ));
    }

    lines.join("\n")
}

/// 每帧刷新战斗页文本内容（属性、日志、技能名、图标占位、血条）。
fn update_battle_ui_system(
    mut text_q: Query<
        (
            &mut Text,
            Option<&BattlePhaseText>,
            Option<&PlayerStatsText>,
            Option<&EnemyStatsText>,
            Option<&LogText>,
            Option<&ResultText>,
            Option<&SkillButtonText>,
            Option<&SkillButtonMetaText>,
            Option<&SkillButtonIconText>,
        ),
    >,
    mut player_hp_fill_q: Query<&mut Node, (With<PlayerHpBarFill>, Without<EnemyHpBarFill>)>,
    mut enemy_hp_fill_q: Query<&mut Node, (With<EnemyHpBarFill>, Without<PlayerHpBarFill>)>,
    player_team: Option<Res<PlayerTeam>>,
    enemy_team: Option<Res<EnemyTeam>>,
    combat_query: Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>,
    skill_query: Query<&SkillList, With<InBattle>>,
    skill_db: Res<SkillDb>,
    battle_log: Res<BattleLog>,
    battle_phase: Res<State<BattlePhase>>,
) {
    let (Some(player_team), Some(enemy_team)) = (player_team, enemy_team) else {
        return;
    };

    let player_line = format_team_roster("玩家队伍", &player_team.0, &combat_query);
    let enemy_line = format_team_roster("敌方队伍", &enemy_team.0, &combat_query);
    let log_line = battle_log
        .0
        .iter()
        .map(|line| format!("• {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut player_skills = None;
    if let Some(p_entity) = player_team.0.active_combatant() {
        if let Ok(skills) = skill_query.get(p_entity) {
            player_skills = Some(skills.0);
        }
    }

    for (
        mut text,
        is_phase,
        is_player,
        is_enemy,
        is_log,
        is_result,
        skill_button_text,
        skill_button_meta_text,
        skill_icon_text,
    ) in &mut text_q
    {
        if is_result.is_some() {
            text.0.clear();
            continue;
        }
        if is_phase.is_some() {
            text.0 = format!("战斗阶段：{}", phase_label(*battle_phase.get()));
            continue;
        }
        if is_player.is_some() {
            text.0 = player_line.clone();
            continue;
        }
        if is_enemy.is_some() {
            text.0 = enemy_line.clone();
            continue;
        }
        if is_log.is_some() {
            text.0 = log_line.clone();
            continue;
        }
        if let (Some(button), Some(skills)) = (skill_button_text, player_skills) {
            let skill_id = skills[button.index];
            text.0 = format!("{}号: {}", button.index + 1, skill_name(skill_id, &skill_db));
            continue;
        }
        if let (Some(meta), Some(skills)) = (skill_button_meta_text, player_skills) {
            let skill_id = skills[meta.index];
            text.0 = skill_meta(skill_id, &skill_db);
            continue;
        }
        if let (Some(icon), Some(_skills)) = (skill_icon_text, player_skills) {
            text.0 = (icon.index + 1).to_string();
        }
    }

    let player_hp_pct = active_hp_percent(&player_team.0, &combat_query);
    let enemy_hp_pct = active_hp_percent(&enemy_team.0, &combat_query);

    if let Ok(mut node) = player_hp_fill_q.single_mut() {
        node.width = Val::Percent(player_hp_pct);
    }
    if let Ok(mut node) = enemy_hp_fill_q.single_mut() {
        node.width = Val::Percent(enemy_hp_pct);
    }
}

/// 更新结果页文本。
fn update_result_ui_system(
    mut result_text_q: Query<&mut Text, With<ResultText>>,
    battle_result: Res<BattleResult>,
) {
    if let Ok(mut result_text) = result_text_q.single_mut() {
        result_text.0 = battle_result.message.clone();
    }
}

/// 从技能库读取技能展示名称。
fn skill_name(skill_id: SkillId, db: &SkillDb) -> String {
    db.0
        .get(&skill_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("{skill_id:?}"))
}

fn skill_meta(skill_id: SkillId, db: &SkillDb) -> String {
    let Some(skill) = db.0.get(&skill_id) else {
        return "类型：未知".to_string();
    };
    match &skill.effect {
        SkillEffect::Attack { .. } => {
            let element_text = match skill.element {
                Some(ElementType::Water) => "·水系",
                Some(ElementType::Fire) => "·火系",
                Some(ElementType::Grass) => "·草系",
                None => "",
            };
            format!("类型：攻击{}", element_text)
        }
        SkillEffect::Heal { .. } => "类型：治疗".to_string(),
        SkillEffect::Shield { .. } => "类型：护盾".to_string(),
    }
}

fn phase_label(phase: BattlePhase) -> &'static str {
    match phase {
        BattlePhase::Init => "初始化",
        BattlePhase::PlayerCommand => "玩家指令",
        BattlePhase::EnemyCommand => "敌方决策",
        BattlePhase::Resolve => "回合结算",
        BattlePhase::CheckEnd => "胜负判定",
    }
}

fn active_hp_percent(team: &Team, query: &Query<(&Combatant, &Stats, &Name, &Shield), With<InBattle>>) -> f32 {
    let Some(entity) = team.active_combatant() else {
        return 0.0;
    };
    let Ok((_, stats, _, _)) = query.get(entity) else {
        return 0.0;
    };
    if stats.max_hp <= 0 {
        return 0.0;
    }
    ((stats.hp.max(0) as f32 / stats.max_hp as f32) * 100.0).clamp(0.0, 100.0)
}

/// 将元素类型转换为中文显示名称。
fn element_name(element: ElementType) -> &'static str {
    match element {
        ElementType::Water => "水",
        ElementType::Fire => "火",
        ElementType::Grass => "草",
    }
}

fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    let mut text_font = TextFont::from_font_size(size);
    if let Some(font) = ui_font {
        text_font.font = font.0.clone();
    }
    text_font
}
