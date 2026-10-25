use crate::models::FilterMode;
use std::path::Path;

const DEFAULT_WINDOWS_BLACKLIST: &[&str] = &[
    "explorer.exe",
    "snipaste.exe",
    "pixpin.exe",
    "sharex.exe",
    "excel.exe",
    "powerpnt.exe",
    "photoshop.exe",
    "illustrator.exe",
    "adobe premiere pro.exe",
    "afterfx.exe",
    "adobe audition.exe",
    "blender.exe",
    "3dsmax.exe",
    "maya.exe",
    "acad.exe",
    "sldworks.exe",
    "mstsc.exe",
];
const DEFAULT_MACOS_BLACKLIST: &[&str] = &["com.apple.finder", "finder"];
pub(super) fn application_allowed(
    mode: &FilterMode,
    custom: &[String],
    app_name: Option<&str>,
    process_name: Option<&str>,
) -> bool {
    let candidates = [app_name, process_name]
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_lowercase())
        .collect::<Vec<_>>();
    if candidates
        .iter()
        .any(|candidate| candidate == "citri" || candidate == "citri.exe")
    {
        return false;
    }
    let matches = |entry: &str| {
        let entry = entry.trim().to_lowercase();
        candidates.iter().any(|candidate| {
            candidate == &entry
                || Path::new(candidate)
                    .file_stem()
                    .and_then(|value| value.to_str())
                    == Some(entry.as_str())
        })
    };
    if *mode == FilterMode::Whitelist {
        return custom.iter().any(|entry| matches(entry));
    }
    let predefined = if cfg!(target_os = "windows") {
        DEFAULT_WINDOWS_BLACKLIST
    } else if cfg!(target_os = "macos") {
        DEFAULT_MACOS_BLACKLIST
    } else {
        &[]
    };
    !custom.iter().any(|entry| matches(entry))
        && !(*mode == FilterMode::Default && predefined.iter().any(|entry| matches(entry)))
}
