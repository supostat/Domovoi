#![allow(dead_code)]

use std::path::{Path, PathBuf};

use domovoi_index::Category;
use domovoi_render::{
    BUILT_METALLIB, DiagramOutline, DiagramRenderer, DiagramStrings, DiagramTree, NodeOrigin, register_fonts,
};
use serde_json::Value;

const GIGABYTE: f64 = 1_000_000_000.0;

pub(crate) const STRINGS: DiagramStrings = DiagramStrings {
    category_names: [
        "Приложения",
        "Разработка",
        "Медиа",
        "Документы",
        "Кэши",
        "Система",
        "Скрытое",
        "Модель",
        "Дом",
    ],
    used: "занято",
    up: "↑ наверх",
};

pub(crate) fn crate_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

pub(crate) fn fonts() {
    register_fonts(&crate_path("../../assets/fonts")).expect("the bundled fonts register");
}

pub(crate) fn book_v() -> DiagramTree {
    let text = std::fs::read_to_string(crate_path("tests/fixtures/book_v.json")).expect("the fixture reads");
    let value: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    DiagramTree::from_outline(&outline(&value))
}

pub(crate) fn renderer() -> DiagramRenderer {
    DiagramRenderer::system_default(Path::new(BUILT_METALLIB)).expect("the diagram renderer starts")
}

fn outline(value: &Value) -> DiagramOutline {
    let category_id = value["category"].as_str().expect("a node has a category");
    DiagramOutline {
        name: value["name"].as_str().expect("a node has a name").to_owned(),
        category: Category::ALL
            .into_iter()
            .find(|category| category.id() == category_id)
            .unwrap_or_else(|| panic!("unknown category {category_id}")),
        size: value["gigabytes"]
            .as_f64()
            .map_or(0, |gigabytes| (gigabytes * GIGABYTE).round() as u64),
        origin: NodeOrigin::Outline,
        children: value["children"]
            .as_array()
            .map(|children| children.iter().map(outline).collect())
            .unwrap_or_default(),
    }
}
