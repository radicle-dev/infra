pub use std::env::{var, var_os, vars, VarError};

/// Pipeline environment variable to be passed through to the build process.
///
/// Must be prefixed by `BUILD_` in the pipeline YAML.
pub fn build_var(suffix: &str) -> Result<String, VarError> {
    var(format!("BUILD_{}", suffix))
}

//// Iterator over all env vars prefixed with `BUILD_`.
pub fn build_vars() -> impl Iterator<Item = (String, String)> {
    vars().filter(|(k, _)| k.starts_with("BUILD_"))
}

/// Iterator over all env vars set by Buildkite as per https://buildkite.com/docs/pipelines/environment-variables
///
/// Variables leaking access credentials are filtered out
pub fn safe_buildkite_vars() -> impl Iterator<Item = (String, String)> {
    vars().filter(|(k, _)| {
        (k.starts_with("BUILDKITE") || k == "CI")
            && k != "BUILDKITE_AGENT_ACCESS_TOKEN"
            && !k.starts_with("BUILDKITE_S3")
    })
}
