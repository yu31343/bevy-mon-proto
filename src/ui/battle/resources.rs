use bevy::prelude::*;

#[derive(Resource, Clone)]
pub(crate) struct UiFontHandle(pub Handle<Font>);

#[derive(Resource, Default, Clone, Copy)]
pub(crate) struct SelectedCard {
    pub index: Option<usize>,
    pub discard_armed: bool,
}

