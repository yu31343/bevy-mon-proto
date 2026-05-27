use crate::battle::Side;

/// 阵营文案转换，用于日志与 UI 展示。
pub(crate) fn side_text(side: Side) -> &'static str {
    match side {
        Side::Player => "玩家",
        Side::Enemy => "敌方",
    }
}

pub(crate) fn element_text(element: crate::data::ElementType) -> &'static str {
    match element {
        crate::data::ElementType::Fire => "火",
        crate::data::ElementType::Water => "水",
        crate::data::ElementType::Grass => "草",
        crate::data::ElementType::Light => "光",
        crate::data::ElementType::Dark => "暗",
        crate::data::ElementType::Thunder => "雷",
        crate::data::ElementType::Wind => "风",
    }
}

pub(crate) fn element_texts(elements: &[crate::data::ElementType]) -> String {
    if elements.is_empty() {
        "无".to_string()
    } else {
        elements
            .iter()
            .map(|element| element_text(*element))
            .collect::<Vec<_>>()
            .join("/")
    }
}

pub(crate) fn status_names(statuses: &crate::battle::StatusBoard) -> String {
    let labels = statuses
        .entries
        .iter()
        .filter(|entry| entry.category != crate::data::StatusCategory::Aura)
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();

    if labels.is_empty() {
        "无".to_string()
    } else {
        labels.join("/")
    }
}
