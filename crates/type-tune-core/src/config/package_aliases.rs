//! Discover and normalize package import and export aliases.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{Result, TypeTuneError};

pub(super) fn normalize_package_module_aliases(
    aliases: BTreeMap<String, Vec<String>>,
    package_scope: &Path,
    project_root: &Path,
    base_url: Option<&str>,
) -> BTreeMap<String, Vec<String>> {
    aliases
        .into_iter()
        .map(|(specifier, targets)| {
            (
                specifier,
                targets
                    .iter()
                    .map(|target| {
                        package_module_alias_path_target(
                            target,
                            package_scope,
                            project_root,
                            base_url,
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

pub(super) fn package_scope_key(package_scope: &Path, project_root: &Path) -> String {
    let scope = package_scope
        .strip_prefix(project_root)
        .unwrap_or_else(|_| Path::new(""));
    let scope = scope.to_string_lossy().replace('\\', "/");
    if scope.is_empty() { ".".into() } else { scope }
}

pub(super) fn package_scope_for_config(config_directory: &Path, project_root: &Path) -> PathBuf {
    let mut directory = Some(config_directory);
    while let Some(current) = directory {
        if current.join("package.json").is_file() {
            return current.to_path_buf();
        }
        if current == project_root {
            break;
        }
        directory = current.parent();
    }
    project_root.to_path_buf()
}

pub(super) fn load_package_module_aliases(
    project_root: &Path,
    custom_conditions: &[String],
    package_require_condition: bool,
    package_node_condition: bool,
) -> Result<BTreeMap<String, Vec<String>>> {
    let package_path = project_root.join("package.json");
    let content = match fs::read_to_string(&package_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(TypeTuneError::new(format!(
                "Could not read {}: {error}",
                package_path.display()
            )));
        }
    };
    let package: serde_json::Value = serde_json::from_str(
        content.strip_prefix('\u{feff}').unwrap_or(&content),
    )
    .map_err(|error| {
        TypeTuneError::new(format!(
            "Could not parse {}: {error}",
            package_path.display()
        ))
    })?;
    let package_require_condition = package_require_condition
        && package.get("type").and_then(serde_json::Value::as_str) != Some("module");
    let mut references = BTreeMap::new();
    if let Some(name) = package.get("name").and_then(serde_json::Value::as_str)
        && let Some(exports) = package.get("exports")
    {
        match package_export_targets(
            exports,
            custom_conditions,
            package_require_condition,
            package_node_condition,
        ) {
            PackageExportTargets::Resolved(targets) if !targets.is_empty() => {
                references.insert(name.into(), targets);
            }
            PackageExportTargets::Resolved(_) if exports.is_object() => {
                for (subpath, export) in exports.as_object().expect("object was checked") {
                    let Some(subpath) = subpath.strip_prefix("./") else {
                        continue;
                    };
                    let PackageExportTargets::Resolved(targets) = package_export_targets(
                        export,
                        custom_conditions,
                        package_require_condition,
                        package_node_condition,
                    ) else {
                        continue;
                    };
                    if targets.is_empty() {
                        continue;
                    }
                    references.insert(format!("{name}/{subpath}"), targets);
                }
            }
            PackageExportTargets::Blocked | PackageExportTargets::Resolved(_) => {}
        }
    }
    if let Some(imports) = package
        .get("imports")
        .and_then(serde_json::Value::as_object)
    {
        for (specifier, import) in imports {
            if !specifier.starts_with('#') {
                continue;
            }
            let PackageExportTargets::Resolved(targets) = package_export_targets(
                import,
                custom_conditions,
                package_require_condition,
                package_node_condition,
            ) else {
                continue;
            };
            if targets.is_empty() {
                continue;
            }
            references.insert(specifier.clone(), targets);
        }
    }
    Ok(references)
}

enum PackageExportTargets {
    Blocked,
    Resolved(Vec<String>),
}

fn package_export_targets(
    export: &serde_json::Value,
    custom_conditions: &[String],
    package_require_condition: bool,
    package_node_condition: bool,
) -> PackageExportTargets {
    if export.is_null() {
        return PackageExportTargets::Blocked;
    }
    if let Some(target) = export.as_str().and_then(normalize_package_export_target) {
        return PackageExportTargets::Resolved(vec![target]);
    }
    if let Some(targets) = export.as_array() {
        let mut resolved = Vec::new();
        for target in targets {
            match package_export_targets(
                target,
                custom_conditions,
                package_require_condition,
                package_node_condition,
            ) {
                PackageExportTargets::Blocked => return PackageExportTargets::Blocked,
                PackageExportTargets::Resolved(targets) => resolved.extend(targets),
            }
        }
        return PackageExportTargets::Resolved(resolved);
    }
    export
        .as_object()
        .and_then(|conditions| {
            conditions.iter().find_map(|(condition, target)| {
                (condition == "types"
                    || (condition == "import" && !package_require_condition)
                    || (condition == "require" && package_require_condition)
                    || (condition == "node" && package_node_condition)
                    || condition == "default"
                    || custom_conditions.iter().any(|custom| custom == condition))
                .then(|| {
                    package_export_targets(
                        target,
                        custom_conditions,
                        package_require_condition,
                        package_node_condition,
                    )
                })
            })
        })
        .unwrap_or_else(|| PackageExportTargets::Resolved(Vec::new()))
}

fn normalize_package_export_target(target: &str) -> Option<String> {
    let target = target.strip_prefix("./")?;
    (!target.split('/').any(|segment| segment == "..")).then(|| target.into())
}

fn package_module_alias_path_target(
    target: &str,
    package_scope: &Path,
    project_root: &Path,
    base_url: Option<&str>,
) -> String {
    let scope_path = package_scope
        .strip_prefix(project_root)
        .unwrap_or_else(|_| Path::new(""));
    let target = if scope_path.as_os_str().is_empty() {
        target.into()
    } else {
        scope_path.join(target).to_string_lossy().replace('\\', "/")
    };
    let Some(base_url) = base_url.filter(|value| !value.is_empty() && *value != ".") else {
        return target;
    };
    let mut base_segments = base_url.split('/').collect::<Vec<_>>();
    let mut target_segments = target.split('/').collect::<Vec<_>>();
    while base_segments.first() == target_segments.first() && !base_segments.is_empty() {
        base_segments.remove(0);
        target_segments.remove(0);
    }
    let mut relative = vec![".."; base_segments.len()];
    relative.extend(target_segments);
    if relative.is_empty() {
        ".".into()
    } else {
        relative.join("/")
    }
}
