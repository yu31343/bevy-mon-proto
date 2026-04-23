use bevy::prelude::*;

#[derive(Resource, Clone)]
pub(crate) struct UiFontHandle(pub Handle<Font>);

