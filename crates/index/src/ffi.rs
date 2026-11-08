use std::ffi::c_void;
use std::io;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_core_foundation::{CFArray, CFDate, CFDictionary, CFRetained, CFString, CFType, CFURL};
use objc2_core_services::{
    MDItem, MDQuery, MDQueryOptionFlags, kMDItemCFBundleIdentifier, kMDItemLastUsedDate, kMDItemPath,
    kMDQueryScopeComputer,
};
use objc2_foundation::{
    NSArray, NSData, NSDictionary, NSNumber, NSPropertyListReadOptions, NSPropertyListSerialization, NSString, NSURL,
    NSURLVolumeAvailableCapacityForImportantUsageKey,
};
use objc2_security::{
    SecCSFlags, SecCode, SecStaticCode, kSecCSSigningInformation, kSecCodeInfoEntitlementsDict,
    kSecCodeInfoTeamIdentifier,
};
use objc2_uniform_type_identifiers::{
    UTType, UTTypeApplicationBundle, UTTypeArchive, UTTypeAudiovisualContent, UTTypeCompositeContent, UTTypeData,
    UTTypeDirectory, UTTypeDiskImage, UTTypeImage, UTTypePDF, UTTypePackage, UTTypeSourceCode, UTTypeText,
};

use crate::code_signing::CodeSigning;
use crate::property_list::{dictionary, text, texts};

const SECONDS_FROM_UNIX_TO_CORE_FOUNDATION_EPOCH: f64 = 978_307_200.0;
const SECURITY_SUCCESS: i32 = 0;
const APPLICATION_GROUPS_ENTITLEMENT: &str = "com.apple.security.application-groups";
const SYNCHRONOUS: objc2_core_foundation::CFOptionFlags =
    MDQueryOptionFlags::Synchronous.0 as objc2_core_foundation::CFOptionFlags;
const LAST_USED_QUERY: &str = "kMDItemLastUsedDate == \"*\"";
const SCREEN_CAPTURE_QUERY: &str = "kMDItemIsScreenCapture == 1";
const APPLICATION_QUERY: &str =
    "kMDItemCFBundleIdentifier == \"*\" && kMDItemContentType == \"com.apple.application-bundle\"";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TypeFamily {
    Image,
    AudioVisual,
    SourceCode,
    Document,
    Archive,
    ApplicationBundle,
}

pub(crate) fn extension_conforms(extension: &str, directory: bool, families: &[TypeFamily]) -> Option<TypeFamily> {
    let base = unsafe { if directory { UTTypeDirectory } else { UTTypeData } };
    let uniform_type = UTType::typeWithFilenameExtension_conformingToType(&NSString::from_str(extension), base)?;
    families.iter().copied().find(|family| {
        family_types(*family)
            .iter()
            .any(|parent| uniform_type.conformsToType(parent))
    })
}

pub(crate) fn names_a_package(name: &std::ffi::OsStr) -> bool {
    let Some(extension) = Path::new(name).extension().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    let directory = unsafe { UTTypeDirectory };
    UTType::typeWithFilenameExtension_conformingToType(&NSString::from_str(extension), directory)
        .is_some_and(|uniform_type| uniform_type.conformsToType(unsafe { UTTypePackage }))
}

fn family_types(family: TypeFamily) -> Vec<&'static UTType> {
    unsafe {
        match family {
            TypeFamily::Image => vec![UTTypeImage],
            TypeFamily::AudioVisual => vec![UTTypeAudiovisualContent],
            TypeFamily::SourceCode => vec![UTTypeSourceCode],
            TypeFamily::Document => vec![UTTypePDF, UTTypeText, UTTypeCompositeContent],
            TypeFamily::Archive => vec![UTTypeArchive, UTTypeDiskImage],
            TypeFamily::ApplicationBundle => vec![UTTypeApplicationBundle],
        }
    }
}

pub(crate) fn spotlight_last_used() -> io::Result<Vec<(PathBuf, SystemTime)>> {
    let date_key = unsafe { required_key(kMDItemLastUsedDate)? };
    Ok(run_query(LAST_USED_QUERY, &[date_key])?
        .into_iter()
        .filter_map(|row| {
            let date = row.values[0].as_deref().and_then(date_value)?;
            Some((row.path?, date))
        })
        .collect())
}

pub(crate) fn spotlight_screen_captures() -> io::Result<Vec<PathBuf>> {
    Ok(run_query(SCREEN_CAPTURE_QUERY, &[])?
        .into_iter()
        .filter_map(|row| row.path)
        .collect())
}

pub(crate) fn spotlight_applications() -> io::Result<Vec<(String, PathBuf)>> {
    let identifier_key = unsafe { required_key(kMDItemCFBundleIdentifier)? };
    Ok(run_query(APPLICATION_QUERY, &[identifier_key])?
        .into_iter()
        .filter_map(|row| {
            let identifier = row.values[0].as_deref().and_then(string_value)?;
            Some((identifier, row.path?))
        })
        .collect())
}

pub(crate) fn important_available_capacity(volume_root: &Path) -> io::Result<u64> {
    let path = volume_root
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "volume path is not UTF-8"))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    let key = unsafe { NSURLVolumeAvailableCapacityForImportantUsageKey };
    let values = url
        .resourceValuesForKeys_error(&NSArray::from_slice(&[key]))
        .map_err(|error| io::Error::other(error.localizedDescription().to_string()))?;
    let value = values
        .objectForKey(key)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "volume reports no important-usage capacity"))?;
    let number = value
        .downcast::<NSNumber>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "important-usage capacity is not a number"))?;
    u64::try_from(number.longLongValue())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "important-usage capacity is negative"))
}

unsafe fn required_key(key: Option<&'static CFString>) -> io::Result<&'static CFString> {
    key.ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "Spotlight attribute key is unavailable"))
}

struct QueryRow {
    path: Option<PathBuf>,
    values: Vec<Option<CFRetained<CFType>>>,
}

fn run_query(query: &str, attributes: &[&CFString]) -> io::Result<Vec<QueryRow>> {
    let path_key = unsafe { required_key(kMDItemPath)? };
    let query_string = CFString::from_str(query);
    let value_list = CFArray::from_objects(attributes);
    let query = unsafe { MDQuery::new(None, Some(&query_string), Some(value_list.as_opaque()), None) }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Spotlight rejected the query"))?;
    let scope = unsafe { kMDQueryScopeComputer }
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "Spotlight scope is unavailable"))?;
    let scopes = CFArray::from_objects(&[scope]);
    unsafe { query.set_search_scope(Some(scopes.as_opaque()), 0) };
    if !unsafe { query.execute(SYNCHRONOUS) } {
        return Err(io::Error::other("Spotlight query failed to run"));
    }
    let count = unsafe { query.result_count() };
    Ok((0..count)
        .map(|index| {
            let item = unsafe { query.result_at_index(index) }.cast::<MDItem>();
            let path = unsafe { item.as_ref() }
                .and_then(|item| unsafe { item.attribute(Some(path_key)) })
                .as_deref()
                .and_then(string_value)
                .map(PathBuf::from);
            let values = attributes
                .iter()
                .map(|attribute| {
                    retained_value(unsafe { query.attribute_value_of_result_at_index(Some(attribute), index) })
                })
                .collect();
            QueryRow { path, values }
        })
        .collect())
}

fn retained_value(value: *mut c_void) -> Option<CFRetained<CFType>> {
    let value = std::ptr::NonNull::new(value.cast::<CFType>())?;
    Some(unsafe { CFRetained::retain(value) })
}

fn string_value(value: &CFType) -> Option<String> {
    value.downcast_ref::<CFString>().map(ToString::to_string)
}

fn date_value(value: &CFType) -> Option<SystemTime> {
    let seconds = value.downcast_ref::<CFDate>()?.absolute_time() + SECONDS_FROM_UNIX_TO_CORE_FOUNDATION_EPOCH;
    (seconds >= 0.0).then(|| UNIX_EPOCH + Duration::from_secs_f64(seconds))
}

pub(crate) fn application_path(bundle_id: &str) -> Option<PathBuf> {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    let url: Retained<NSURL> = workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
    url.path().map(|path| PathBuf::from(path.to_string()))
}

pub(crate) fn property_list_dictionary(path: &Path) -> Option<Retained<NSDictionary<AnyObject, AnyObject>>> {
    let data = NSData::dataWithContentsOfFile(&NSString::from_str(path.to_str()?))?;
    let plist = unsafe {
        NSPropertyListSerialization::propertyListWithData_options_format_error(
            &data,
            NSPropertyListReadOptions::empty(),
            ptr::null_mut(),
        )
    }
    .ok()?;
    plist.downcast::<NSDictionary<AnyObject, AnyObject>>().ok()
}

pub(crate) fn code_signing(bundle: &Path) -> Option<CodeSigning> {
    let url = CFURL::from_directory_path(bundle)?;
    let mut code: *const SecStaticCode = ptr::null();
    let created = unsafe { SecStaticCode::create_with_path(&url, SecCSFlags::DefaultFlags, NonNull::from(&mut code)) };
    if created != SECURITY_SUCCESS {
        return None;
    }
    let code = unsafe { CFRetained::from_raw(NonNull::new(code.cast_mut())?) };
    let mut information: *const CFDictionary = ptr::null();
    let copied = unsafe {
        SecCode::copy_signing_information(
            &code,
            SecCSFlags(kSecCSSigningInformation),
            NonNull::from(&mut information),
        )
    };
    if copied != SECURITY_SUCCESS {
        return None;
    }
    let information: Retained<NSDictionary<AnyObject, AnyObject>> =
        unsafe { Retained::from_raw(information.cast_mut().cast())? };
    let team_key = unsafe { bridged(kSecCodeInfoTeamIdentifier) };
    let entitlements_key = unsafe { bridged(kSecCodeInfoEntitlementsDict) };
    let app_groups = dictionary(&information, entitlements_key).map_or_else(Vec::new, |entitlements| {
        texts(&entitlements, &NSString::from_str(APPLICATION_GROUPS_ENTITLEMENT))
    });
    Some(CodeSigning {
        team_id: text(&information, team_key),
        app_groups,
    })
}

unsafe fn bridged(key: &'static CFString) -> &'static NSString {
    unsafe { &*ptr::from_ref(key).cast::<NSString>() }
}
