use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

use crate::category::Category;
use crate::ffi::{TypeFamily, extension_conforms};

const DIRECTORY_FAMILIES: [TypeFamily; 1] = [TypeFamily::ApplicationBundle];
const FILE_FAMILIES: [TypeFamily; 5] = [
    TypeFamily::SourceCode,
    TypeFamily::Image,
    TypeFamily::AudioVisual,
    TypeFamily::Document,
    TypeFamily::Archive,
];

#[derive(Debug, Default)]
pub(crate) struct FileTypes {
    known: HashMap<(String, bool), Option<Category>>,
}

impl FileTypes {
    pub(crate) fn category(&mut self, name: &OsStr, directory: bool) -> Option<Category> {
        let extension = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
        *self
            .known
            .entry((extension, directory))
            .or_insert_with_key(|(extension, directory)| {
                let families: &[TypeFamily] = if *directory {
                    &DIRECTORY_FAMILIES
                } else {
                    &FILE_FAMILIES
                };
                extension_conforms(extension, *directory, families).map(|family| match family {
                    TypeFamily::ApplicationBundle => Category::Apps,
                    TypeFamily::SourceCode => Category::Dev,
                    TypeFamily::Image | TypeFamily::AudioVisual => Category::Media,
                    TypeFamily::Document | TypeFamily::Archive => Category::Docs,
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::FileTypes;
    use crate::category::Category;

    #[test]
    fn classifies_by_the_system_type_of_the_extension() {
        let mut types = FileTypes::default();
        assert_eq!(types.category(OsStr::new("holiday.HEIC"), false), Some(Category::Media));
        assert_eq!(types.category(OsStr::new("talk.mov"), false), Some(Category::Media));
        assert_eq!(types.category(OsStr::new("song.mp3"), false), Some(Category::Media));
        assert_eq!(types.category(OsStr::new("report.pdf"), false), Some(Category::Docs));
        assert_eq!(types.category(OsStr::new("installer.dmg"), false), Some(Category::Docs));
        assert_eq!(types.category(OsStr::new("main.c"), false), Some(Category::Dev));
        assert_eq!(types.category(OsStr::new("Notes.app"), true), Some(Category::Apps));
        assert_eq!(types.category(OsStr::new("no-extension"), false), None);
    }
}
