use bevy::prelude::*;
use std::{
    fs,
};

use crate::{
    battle::{BattleLog, BattleResult, Combatant, InBattle, Side, SkillList, Stats, TurnContext},
    data::{SkillDb, SkillId},
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

/// UI 插件：负责渲染和更新战斗界面。
pub struct UiPlugin;

#[derive(Resource, Clone)]
struct UiFontHandle(Handle<Font>);

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_camera, load_cjk_font_system, setup_ui_system).chain())
            .add_systems(
                Update,
                (
                    button_select_skill_system
                        .run_if(in_state(GameState::Battle).and(in_state(BattlePhase::PlayerCommand))),
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

/// 创建战斗 UI 节点树（属性、技能栏、日志、结算文本）。
fn setup_ui_system(mut commands: Commands, ui_font: Option<Res<UiFontHandle>>) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 1.0)),
        ))
        .with_children(|root| {
            let title_font = make_text_font(26.0, ui_font.as_deref());
            let body_font = make_text_font(20.0, ui_font.as_deref());
            let log_font = make_text_font(18.0, ui_font.as_deref());
            let result_font = make_text_font(28.0, ui_font.as_deref());

            root.spawn((
                Text::new("玩家：..."),
                title_font.clone(),
                TextColor(Color::WHITE),
                PlayerStatsText,
            ));
            root.spawn((
                Text::new("敌方：..."),
                title_font,
                TextColor(Color::WHITE),
                EnemyStatsText,
            ));

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(72.0),
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|row| {
                for idx in 0..4 {
                    row.spawn((
                        Button,
                        Node {
                            width: Val::Px(200.0),
                            height: Val::Px(48.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 1.0)),
                        SkillButton { index: idx },
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(format!("技能 {}", idx + 1)),
                            body_font.clone(),
                            TextColor(Color::WHITE),
                            SkillButtonText { index: idx },
                        ));
                    });
                }
            });

            root.spawn((
                Text::new("日志："),
                body_font,
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));

            root.spawn((
                Text::new(""),
                log_font,
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
                LogText,
            ));

            root.spawn((
                Text::new(""),
                result_font,
                TextColor(Color::srgb(1.0, 0.8, 0.2)),
                ResultText,
            ));
        });
}

/// 处理技能按钮点击，行为与键盘选招保持一致。
fn button_select_skill_system(
    mut interaction_query: Query<(&Interaction, &SkillButton), (Changed<Interaction>, With<Button>)>,
    mut turn_ctx: ResMut<TurnContext>,
    query: Query<(&Combatant, &SkillList), With<InBattle>>,
    mut next_phase: ResMut<NextState<BattlePhase>>,
) {
    let mut player_skills = None;
    for (combatant, skills) in &query {
        if combatant.side == Side::Player {
            player_skills = Some(skills.0);
            break;
        }
    }
    let Some(skills) = player_skills else {
        return;
    };

    for (interaction, button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            turn_ctx.player_skill = Some(skills[button.index]);
            next_phase.set(BattlePhase::EnemyCommand);
        }
    }
}

/// 每帧刷新战斗页文本内容（属性、日志、技能名）。
fn update_battle_ui_system(
    mut text_q: Query<
        (
            &mut Text,
            Option<&PlayerStatsText>,
            Option<&EnemyStatsText>,
            Option<&LogText>,
            Option<&ResultText>,
            Option<&SkillButtonText>,
        ),
    >,
    query: Query<(&Combatant, &Stats, &SkillList), With<InBattle>>,
    skill_db: Res<SkillDb>,
    battle_log: Res<BattleLog>,
) {
    let mut player_line = String::from("玩家：...");
    let mut enemy_line = String::from("敌方：...");
    for (combatant, stats, _) in &query {
        match combatant.side {
            Side::Player => {
                player_line = format!(
                    "玩家 生命: {}/{}  攻击:{} 防御:{} 速度:{}",
                    stats.hp, stats.max_hp, stats.atk, stats.def, stats.spd
                );
            }
            Side::Enemy => {
                enemy_line = format!(
                    "敌方 生命: {}/{}  攻击:{} 防御:{} 速度:{}",
                    stats.hp, stats.max_hp, stats.atk, stats.def, stats.spd
                );
            }
        }
    }

    let log_line = battle_log.0.iter().cloned().collect::<Vec<_>>().join("\n");

    let mut player_skills = None;
    for (combatant, _, skills) in &query {
        if combatant.side == Side::Player {
            player_skills = Some(skills.0);
            break;
        }
    }

    for (mut text, is_player, is_enemy, is_log, is_result, skill_button_text) in &mut text_q {
        if is_result.is_some() {
            text.0.clear();
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
            let display_name = skill_name(skill_id, &skill_db);
            text.0 = format!("{}号: {}", button.index + 1, display_name);
        }
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

fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    let mut text_font = TextFont::from_font_size(size);
    if let Some(font) = ui_font {
        text_font.font = font.0.clone();
    }
    text_font
}
