use std::path::Path;

use objc2_foundation::NSString;

use crate::ffi::property_list_dictionary;
use crate::property_list::text;

const METADATA_FILE: &str = ".com.apple.containermanagerd.metadata.plist";
const IDENTIFIER_KEY: &str = "MCMMetadataIdentifier";

pub(crate) fn container_identifier(container: &Path) -> Option<String> {
    let metadata = property_list_dictionary(&container.join(METADATA_FILE))?;
    text(&metadata, &NSString::from_str(IDENTIFIER_KEY))
}
