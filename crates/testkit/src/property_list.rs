use std::fmt::Write;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyValue<'text> {
    Text(&'text str),
    Texts(&'text [&'text str]),
}

pub fn write_property_list(path: &Path, entries: &[(&str, PropertyValue<'_>)]) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    fs::write(path, document(entries))
}

fn document(entries: &[(&str, PropertyValue<'_>)]) -> String {
    let mut body = String::new();
    for (key, value) in entries {
        write!(body, "<key>{key}</key>").expect("a string takes the key");
        match value {
            PropertyValue::Text(text) => write!(body, "<string>{text}</string>").expect("a string takes the value"),
            PropertyValue::Texts(texts) => {
                body.push_str("<array>");
                for text in *texts {
                    write!(body, "<string>{text}</string>").expect("a string takes the item");
                }
                body.push_str("</array>");
            }
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>{body}</dict></plist>\n"
    )
}
