use domovoi_index::Category;
use domovoi_render::{DiagramOutline, NodeOrigin};
use serde::Deserialize;

const GIGABYTE: f64 = 1_000_000_000.0;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlineFixture {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub gigabytes: Option<f64>,
    #[serde(default)]
    pub children: Vec<OutlineFixture>,
}

impl OutlineFixture {
    pub fn outline(&self) -> Result<DiagramOutline, String> {
        let category = Category::ALL
            .into_iter()
            .find(|category| category.id() == self.category)
            .ok_or_else(|| format!("unknown category {:?}", self.category))?;
        Ok(DiagramOutline {
            name: self.name.clone(),
            category,
            size: self
                .gigabytes
                .map_or(0, |gigabytes| (gigabytes * GIGABYTE).round() as u64),
            origin: NodeOrigin::Outline,
            children: self.children.iter().map(Self::outline).collect::<Result<_, _>>()?,
        })
    }
}
