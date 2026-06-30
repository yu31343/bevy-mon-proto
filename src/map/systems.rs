use crate::console_log::{ConsoleLogCategory, log as console_log};
use crate::data::{MapBattleContext, MonsterPool};
use crate::game_state::GameState;
use crate::map::components::{
    Character, CurrentMap, EnterLobbyButton, Map, MapNameBannerLine, MapNameBannerRoot,
    MapNameBannerText, MapNameBannerTimer, MapNavigationButton, MapSpriteOutline, MapUiRoot,
    SpriteEntity, SpriteNameLabel,
};
use crate::team_selection::SelectionEntryMode;
use crate::ui::battle::{resources::UiFontHandle, theme::UiTheme};
use bevy::prelude::*;

const PLAYER_SIZE: Vec2 = Vec2::new(160.0, 160.0);
const WATER_SPRITE_SIZE: Vec2 = Vec2::new(90.0, 90.0);
const GRASS_SPRITE_SIZE: Vec2 = Vec2::new(280.0, 180.0);
const FIRE_SPRITE_SIZE: Vec2 = Vec2::new(200.0, 200.0);
const DARK_SPRITE_SIZE: Vec2 = Vec2::new(160.0, 160.0);
const LIGHT_SPRITE_SIZE: Vec2 = Vec2::new(160.0, 160.0);
const THUNDER_SPRITE_SIZE: Vec2 = Vec2::new(160.0, 160.0);
const WIND_SPRITE_SIZE: Vec2 = Vec2::new(160.0, 160.0);
const MAP_SPRITE_OUTLINE_WIDTH: f32 = 6.0;
const MAP_SPRITE_NAME_GAP: f32 = 16.0;
const MAP_SPRITE_NAME_OUTLINE_WIDTH: f32 = 2.0;

pub fn spawn_map(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    current_map: Res<CurrentMap>,
) {
    commands.spawn((
        Sprite {
            image: asset_server.load(current_map.asset_path()),
            custom_size: Some(Vec2::new(1200.0, 800.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        Map,
    ));
}

pub fn spawn_character(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    ui_font: Option<Res<UiFontHandle>>,
) {
    let image = asset_server.load("map/player.png");

    commands
        .spawn((
            Sprite {
                image: image.clone(),
                custom_size: Some(PLAYER_SIZE),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 2.0),
            Character {
                speed: 100.0,
                target_position: None,
            },
        ))
        .with_children(|parent| {
            spawn_sprite_outline(
                parent,
                image,
                PLAYER_SIZE,
                Color::srgba(0.0, 0.25, 1.0, 0.75),
            );
            spawn_sprite_name(parent, "玩家", PLAYER_SIZE, ui_font.as_deref());
        });
}

pub fn setup_sprite_spine(
    _commands: Commands,
    // 暂时简化，稍后添加Spine
) {
    // TODO: 集成Spine动画
}

pub fn spawn_sprites(
    mut commands: Commands,
    monster_pool: Res<MonsterPool>,
    ui_font: Option<Res<UiFontHandle>>,
    asset_server: Res<AssetServer>,
    current_map: Res<CurrentMap>,
) {
    spawn_map_sprites(
        &mut commands,
        monster_pool.as_ref(),
        ui_font.as_deref(),
        asset_server.as_ref(),
        *current_map,
    );
}

fn spawn_map_sprites(
    commands: &mut Commands,
    monster_pool: &MonsterPool,
    ui_font: Option<&UiFontHandle>,
    asset_server: &AssetServer,
    current_map: CurrentMap,
) {
    match current_map {
        CurrentMap::Map1 => {
            let configs = [
                ("水精灵", Vec3::new(100.0, 20.0, 1.0)),
                ("草精灵", Vec3::new(-300.0, -160.0, 1.0)),
                ("火精灵", Vec3::new(300.0, -200.0, 1.0)),
            ];
            spawn_configured_sprites(commands, monster_pool, ui_font, asset_server, &configs);
        }
        CurrentMap::Map2 => {
            let configs = [
                ("光精灵", Vec3::new(-250.0, 80.0, 1.0)),
                ("光精灵", Vec3::new(250.0, 80.0, 1.0)),
                ("风精灵", Vec3::new(-250.0, -180.0, 1.0)),
                ("风精灵", Vec3::new(250.0, -180.0, 1.0)),
            ];
            spawn_configured_sprites(commands, monster_pool, ui_font, asset_server, &configs);
        }
        CurrentMap::Map3 => {
            let configs = [
                ("暗精灵", Vec3::new(-250.0, 80.0, 1.0)),
                ("暗精灵", Vec3::new(250.0, 80.0, 1.0)),
                ("雷精灵", Vec3::new(-250.0, -180.0, 1.0)),
                ("雷精灵", Vec3::new(250.0, -180.0, 1.0)),
            ];
            spawn_configured_sprites(commands, monster_pool, ui_font, asset_server, &configs);
        }
    }
}

fn spawn_configured_sprites(
    commands: &mut Commands,
    monster_pool: &MonsterPool,
    ui_font: Option<&UiFontHandle>,
    asset_server: &AssetServer,
    configs: &[(&str, Vec3)],
) {
    for (monster_name, pos) in configs {
        let Some((monster_index, monster)) = monster_pool
            .monsters
            .iter()
            .enumerate()
            .find(|(_, monster)| monster.name == *monster_name)
        else {
            continue;
        };
        let (image_path, sprite_size) = map_sprite_visuals(monster.name.as_str());
        let image = asset_server.load(image_path);

        commands
            .spawn((
                Sprite {
                    image: image.clone(),
                    custom_size: Some(sprite_size),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 2.0),
                SpriteEntity {
                    monster_type: monster.name.clone(),
                    monster_index,
                    click_radius: sprite_size.x.max(sprite_size.y) / 2.0,
                },
            ))
            .with_children(|parent| {
                spawn_sprite_outline(
                    parent,
                    image,
                    sprite_size,
                    Color::srgba(1.0, 0.0, 0.0, 0.75),
                );
                spawn_sprite_name(parent, monster.name.as_str(), sprite_size, ui_font);
            });
    }
}

fn map_sprite_visuals(monster_name: &str) -> (&'static str, Vec2) {
    match monster_name {
        "水精灵" => ("map/water.png", WATER_SPRITE_SIZE),
        "草精灵" => ("map/grass.png", GRASS_SPRITE_SIZE),
        "火精灵" => ("map/fire.png", FIRE_SPRITE_SIZE),
        "暗精灵" => ("map/dark.png", DARK_SPRITE_SIZE),
        "光精灵" => ("map/light.png", LIGHT_SPRITE_SIZE),
        "雷精灵" => ("map/thunder.png", THUNDER_SPRITE_SIZE),
        "风精灵" => ("map/wind.png", WIND_SPRITE_SIZE),
        _ => ("map/water.png", WATER_SPRITE_SIZE),
    }
}

pub fn setup_map_ui(
    mut commands: Commands,
    theme: Res<UiTheme>,
    ui_font: Option<Res<UiFontHandle>>,
    current_map: Res<CurrentMap>,
    existing_ui: Query<(), With<MapUiRoot>>,
    mut banner_timer: ResMut<MapNameBannerTimer>,
) {
    if !existing_ui.is_empty() {
        return;
    }

    // 进入地图系统时触发地图名横幅
    banner_timer.timer.reset();

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Start,
                align_items: AlignItems::Start,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            MapUiRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Button,
                Node {
                    width: Val::Px(180.0),
                    min_height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                EnterLobbyButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("返回大厅"),
                    make_text_font(22.0, ui_font.as_deref()),
                    TextColor(theme.text_primary),
                ));
            });

            root.spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(40.0),
                    bottom: Val::Px(180.0),
                    width: Val::Px(150.0),
                    min_height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                if *current_map == CurrentMap::Map1 {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                },
                MapNavigationButton {
                    target: CurrentMap::Map1,
                },
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new(CurrentMap::Map1.name()),
                    make_text_font(24.0, ui_font.as_deref()),
                    TextColor(theme.text_primary),
                ));
            });

            root.spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(40.0),
                    bottom: Val::Px(110.0),
                    width: Val::Px(150.0),
                    min_height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                if *current_map == CurrentMap::Map2 {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                },
                MapNavigationButton {
                    target: CurrentMap::Map2,
                },
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new(CurrentMap::Map2.name()),
                    make_text_font(24.0, ui_font.as_deref()),
                    TextColor(theme.text_primary),
                ));
            });

            root.spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(40.0),
                    bottom: Val::Px(40.0),
                    width: Val::Px(150.0),
                    min_height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(theme.radius_button),
                    ..default()
                },
                BackgroundColor(theme.button_idle),
                BorderColor::all(theme.button_border_idle),
                theme.button_shadow(),
                if *current_map == CurrentMap::Map3 {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                },
                MapNavigationButton {
                    target: CurrentMap::Map3,
                },
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new(CurrentMap::Map3.name()),
                    make_text_font(24.0, ui_font.as_deref()),
                    TextColor(theme.text_primary),
                ));
            });

            // === 地图名横幅：透明背景，横线+文字+横线，从顶部滑入 ===
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    right: Val::Percent(0.0),
                    top: Val::Percent(-15.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(24.0),
                    display: Display::None,
                    ..default()
                },
                MapNameBannerRoot,
                ZIndex(100),
            ))
            .with_children(|banner| {
                // 左侧横线
                banner.spawn((
                    Node {
                        width: Val::Px(0.0),
                        height: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    MapNameBannerLine,
                ));
                // 地图名文字（浅金色系，非纯白）
                banner.spawn((
                    Text::new(current_map.name()),
                    make_text_font(72.0, ui_font.as_deref()),
                    TextColor(Color::NONE),
                    TextShadow {
                        offset: Vec2::new(3.0, 3.0),
                        color: Color::srgba(0.0, 0.0, 0.0, 0.65),
                    },
                    MapNameBannerText,
                ));
                // 右侧横线
                banner.spawn((
                    Node {
                        width: Val::Px(0.0),
                        height: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    MapNameBannerLine,
                ));
            });
        });
}

pub fn move_character(
    time: Res<Time>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut query: Query<(&mut Transform, &mut Character)>,
) {
    // 设置目标位置
    if mouse_button_input.just_pressed(MouseButton::Left) {
        if let Ok(window) = windows.single() {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_q.single() {
                    if let Ok(world_pos) = camera.viewport_to_world(camera_transform, cursor_pos) {
                        let world_pos_2d: Vec2 = world_pos.origin.truncate();
                        // 边界检查
                        let clamped_pos = Vec2::new(
                            world_pos_2d.x.clamp(-400.0 + 10.0, 400.0 - 10.0),
                            world_pos_2d.y.clamp(-300.0 + 10.0, 300.0 - 10.0),
                        );
                        for (_, mut character) in query.iter_mut() {
                            character.target_position = Some(clamped_pos);
                        }
                    }
                }
            }
        }
    }

    // 移动到目标
    for (mut transform, mut character) in query.iter_mut() {
        if let Some(target) = character.target_position {
            let current_pos = transform.translation.truncate();
            let direction = (target - current_pos).normalize();
            let distance = (target - current_pos).length();

            if distance > 5.0 {
                // 到达阈值
                let move_distance = character.speed * time.delta_secs();
                let new_pos = current_pos + direction * move_distance.min(distance);

                // 再次边界检查
                let clamped_pos = Vec2::new(
                    new_pos.x.clamp(-400.0 + 10.0, 400.0 - 10.0),
                    new_pos.y.clamp(-300.0 + 10.0, 300.0 - 10.0),
                );

                transform.translation.x = clamped_pos.x;
                transform.translation.y = clamped_pos.y;
            } else {
                character.target_position = None; // 到达目标
            }
        }
    }
}

pub fn click_sprites(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    sprite_q: Query<(&Transform, &SpriteEntity), With<SpriteEntity>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut entry_mode: ResMut<SelectionEntryMode>,
    mut map_battle_context: ResMut<MapBattleContext>,
    current_map: Res<CurrentMap>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        if let Ok(window) = windows.single() {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_q.single() {
                    if let Ok(world_pos) = camera.viewport_to_world(camera_transform, cursor_pos) {
                        let world_pos: Vec2 = world_pos.origin.truncate(); // 2D
                        for (transform, sprite) in sprite_q.iter() {
                            let distance = (transform.translation.truncate() - world_pos).length();
                            if distance < sprite.click_radius {
                                console_log(
                                    ConsoleLogCategory::Map,
                                    format!(
                                        "点击地图怪物：{}（index={}）",
                                        sprite.monster_type, sprite.monster_index
                                    ),
                                );
                                map_battle_context.enemy_monster_index = Some(sprite.monster_index);
                                map_battle_context.return_map = Some(*current_map);
                                *entry_mode = SelectionEntryMode::VsAi;
                                next_state.set(GameState::TeamSelection);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn map_enter_lobby_button_system(
    mut next_state: ResMut<NextState<GameState>>,
    mut map_battle_context: ResMut<MapBattleContext>,
    mut button_query: Query<
        &Interaction,
        (Changed<Interaction>, With<Button>, With<EnterLobbyButton>),
    >,
) {
    for interaction in &mut button_query {
        if *interaction == Interaction::Pressed {
            *map_battle_context = MapBattleContext::default();
            next_state.set(GameState::Lobby);
        }
    }
}

pub fn map_navigation_button_system(
    mut current_map: ResMut<CurrentMap>,
    mut banner_timer: ResMut<MapNameBannerTimer>,
    mut button_query: Query<
        (&Interaction, &MapNavigationButton),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, button) in &mut button_query {
        if *interaction == Interaction::Pressed {
            *current_map = button.target;
            // 切换到另一张地图时重新触发地图名横幅
            banner_timer.timer.reset();
        }
    }
}

pub fn update_map_background_system(
    mut commands: Commands,
    current_map: Res<CurrentMap>,
    asset_server: Res<AssetServer>,
    monster_pool: Res<MonsterPool>,
    ui_font: Option<Res<UiFontHandle>>,
    mut map_query: Query<&mut Sprite, With<Map>>,
    monster_entities: Query<Entity, With<SpriteEntity>>,
    mut navigation_buttons: Query<(&MapNavigationButton, &mut Visibility)>,
) {
    if !current_map.is_changed() {
        return;
    }

    for mut sprite in &mut map_query {
        sprite.image = asset_server.load(current_map.asset_path());
    }

    for entity in &monster_entities {
        commands.entity(entity).despawn();
    }

    spawn_map_sprites(
        &mut commands,
        monster_pool.as_ref(),
        ui_font.as_deref(),
        asset_server.as_ref(),
        *current_map,
    );

    for (button, mut visibility) in &mut navigation_buttons {
        *visibility = if button.target == *current_map {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

pub fn map_button_visual_system(
    theme: Res<UiTheme>,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (
            Changed<Interaction>,
            With<Button>,
            Or<(With<EnterLobbyButton>, With<MapNavigationButton>)>,
        ),
    >,
) {
    for (interaction, mut background, mut border) in &mut interaction_query {
        *background = match *interaction {
            Interaction::Pressed => BackgroundColor(theme.button_pressed),
            Interaction::Hovered => BackgroundColor(theme.button_hover),
            Interaction::None => BackgroundColor(theme.button_idle),
        };

        *border = match *interaction {
            Interaction::Pressed => BorderColor::all(theme.button_border_pressed),
            Interaction::Hovered => BorderColor::all(theme.button_border_hover),
            Interaction::None => BorderColor::all(theme.button_border_idle),
        };
    }
}

pub fn map_name_banner_system(
    time: Res<Time>,
    current_map: Res<CurrentMap>,
    mut banner_timer: ResMut<MapNameBannerTimer>,
    mut root_q: Query<&mut Node, With<MapNameBannerRoot>>,
    mut text_q: Query<(&mut Text, &mut TextColor), With<MapNameBannerText>>,
    mut line_q: Query<
        (&mut Node, &mut BackgroundColor),
        (With<MapNameBannerLine>, Without<MapNameBannerRoot>),
    >,
) {
    if banner_timer.timer.elapsed() >= banner_timer.timer.duration() {
        for mut node in &mut root_q {
            node.display = Display::None;
        }
        return;
    }

    banner_timer.timer.tick(time.delta());

    // 总时长 2.0s：0.0~0.5 滑入，0.5~1.5 保持，1.5~2.0 滑出
    let elapsed = banner_timer.timer.elapsed().as_secs_f32();
    let total = banner_timer.timer.duration().as_secs_f32();

    // 位置：渐入时 top 从 -15% 滑到 18%，渐出时从 18% 滑到 -15%
    let target_top = 18.0;
    let offscreen_top = -15.0;
    let top = if elapsed <= 0.5 {
        let t = smoothstep(0.0, 0.5, elapsed);
        offscreen_top + (target_top - offscreen_top) * t
    } else if elapsed <= 1.5 {
        target_top
    } else if elapsed < total {
        let t = smoothstep(1.5, 2.0, elapsed);
        target_top + (offscreen_top - target_top) * t
    } else {
        offscreen_top
    };

    // 透明度：渐入 0→1，保持 1，渐出 1→0
    let alpha = if elapsed <= 0.5 {
        smoothstep(0.0, 0.5, elapsed)
    } else if elapsed <= 1.5 {
        1.0
    } else if elapsed < total {
        smoothstep(2.0, 1.5, elapsed)
    } else {
        0.0
    };

    // 横线宽度：渐入 0→120，保持 120，渐出 120→0
    let line_width = if elapsed <= 0.5 {
        120.0 * smoothstep(0.0, 0.5, elapsed)
    } else if elapsed <= 1.5 {
        120.0
    } else if elapsed < total {
        120.0 * smoothstep(2.0, 1.5, elapsed)
    } else {
        0.0
    };

    // 浅金色系文字颜色（非纯白）
    let text_color = Color::srgba(0.96, 0.90, 0.76, alpha);
    let line_color = Color::srgba(0.96, 0.90, 0.76, alpha * 0.8);

    for mut node in &mut root_q {
        node.display = Display::Flex;
        node.top = Val::Percent(top);
    }

    for (mut text, mut color) in &mut text_q {
        if text.as_str() != current_map.name() {
            *text = Text::new(current_map.name());
        }
        *color = TextColor(text_color);
    }

    for (mut line_node, mut line_bg) in &mut line_q {
        line_node.width = Val::Px(line_width);
        *line_bg = BackgroundColor(line_color);
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn cleanup_map(
    mut commands: Commands,
    query: Query<
        Entity,
        Or<(
            With<Map>,
            With<Character>,
            With<SpriteEntity>,
            With<SpriteNameLabel>,
            With<MapSpriteOutline>,
            With<MapNavigationButton>,
            With<MapUiRoot>,
        )>,
    >,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn spawn_sprite_outline(
    parent: &mut ChildSpawnerCommands,
    image: Handle<Image>,
    sprite_size: Vec2,
    outline_color: Color,
) {
    parent.spawn((
        Sprite {
            image,
            color: outline_color,
            custom_size: Some(sprite_size + Vec2::splat(MAP_SPRITE_OUTLINE_WIDTH * 2.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -0.1),
        MapSpriteOutline,
    ));
}

fn spawn_sprite_name(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    sprite_size: Vec2,
    ui_font: Option<&UiFontHandle>,
) {
    let y = -(sprite_size.y / 2.0 + MAP_SPRITE_NAME_GAP);
    let outline_offsets = [
        Vec2::new(-1.0, -1.0),
        Vec2::new(-1.0, 0.0),
        Vec2::new(-1.0, 1.0),
        Vec2::new(0.0, -1.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
    ];

    for offset in outline_offsets {
        parent.spawn((
            Text2d::new(name.to_string()),
            make_text_font(20.0, ui_font),
            TextColor(Color::WHITE),
            Transform::from_xyz(
                offset.x * MAP_SPRITE_NAME_OUTLINE_WIDTH,
                y + offset.y * MAP_SPRITE_NAME_OUTLINE_WIDTH,
                0.9,
            ),
            SpriteNameLabel,
        ));
    }

    parent.spawn((
        Text2d::new(name.to_string()),
        make_text_font(20.0, ui_font),
        TextColor(Color::BLACK),
        Transform::from_xyz(0.0, y, 1.0),
        SpriteNameLabel,
    ));
}

fn make_text_font(size: f32, ui_font: Option<&UiFontHandle>) -> TextFont {
    if let Some(ui_font) = ui_font {
        TextFont {
            font: ui_font.0.clone(),
            font_size: size,
            ..default()
        }
    } else {
        TextFont {
            font_size: size,
            ..default()
        }
    }
}
