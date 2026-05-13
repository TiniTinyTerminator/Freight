pub mod git;
pub mod http;
pub mod vcpkg;

pub use git::*;
pub use http::fetch_url_dep;
pub use vcpkg::fetch_vcpkg_dep;
