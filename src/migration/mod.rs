pub mod autotools;
pub mod cmake;
pub mod make;

/// TOML version for a discovered system dependency. The source build files carry
/// no version, so freight asks pkg-config for the installed `--modversion` and
/// pins that. When the library isn't found via pkg-config the version is unknown;
/// it falls back to `"*"` as a draft placeholder, which `freight build` then
/// rejects (freight forbids a bare `*`), prompting the user to pin it.
pub(crate) fn system_dep_item(name: &str) -> toml_edit::Item {
    let version = crate::adaptors::pkg_config_version(name);
    let v = if version.is_empty() { "*" } else { &version };
    toml_edit::value(v)
}

/// Normalise a foreign build-system target name into a freight-safe package
/// name: trim leading/trailing non-alphanumerics, then replace every character
/// that isn't `[A-Za-z0-9_-]` with `-`. Shared by the Make and autotools
/// migrators. (The CMake migrator additionally lower-cases — see
/// `cmake::sanitize_name`.)
pub(crate) fn sanitize_name(s: &str) -> String {
    s.trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}
