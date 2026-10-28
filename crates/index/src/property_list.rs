use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSArray, NSDictionary, NSString};

pub(crate) fn text(dictionary: &NSDictionary<AnyObject, AnyObject>, key: &NSString) -> Option<String> {
    let value = dictionary.objectForKey(key)?;
    value.downcast::<NSString>().ok().map(|text| text.to_string())
}

pub(crate) fn texts(dictionary: &NSDictionary<AnyObject, AnyObject>, key: &NSString) -> Vec<String> {
    let Some(value) = dictionary.objectForKey(key) else {
        return Vec::new();
    };
    match value.downcast::<NSString>() {
        Ok(single) => vec![single.to_string()],
        Err(value) => value.downcast::<NSArray>().map_or_else(
            |_| Vec::new(),
            |array| {
                array
                    .iter()
                    .filter_map(|item| item.downcast::<NSString>().ok().map(|text| text.to_string()))
                    .collect()
            },
        ),
    }
}

pub(crate) fn dictionary(
    dictionary: &NSDictionary<AnyObject, AnyObject>,
    key: &NSString,
) -> Option<Retained<NSDictionary<AnyObject, AnyObject>>> {
    let value = dictionary.objectForKey(key)?;
    value.downcast::<NSDictionary>().ok()
}
