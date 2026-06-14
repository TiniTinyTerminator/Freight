pub mod autotools;
pub mod cmake;
pub mod make;

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
