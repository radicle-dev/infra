use buildkite_hooks::config::Config;
use std::path::PathBuf;

/// This command prints a list of `KEY=value` pairs that set the buildkite
/// environment.
///
/// At the moment only the `BUILDKITE_ARTIFACT_UPLOAD_DESTINATION` is set.
///
/// This command is called by the `hooks/environment` shell script.
#[paw::main]
fn main(cfg: Config) {
    println!(
        "BUILDKITE_ARTIFACT_UPLOAD_DESTINATION={}",
        upload_destination(&cfg)
    );
}

fn upload_destination(cfg: &Config) -> String {
    let mut dest = PathBuf::from("builds.radicle.xyz/");
    dest.push(cfg.buildkite_pipeline_slug.clone());

    if let Some(tag) = &*cfg.tag {
        dest.push(tag);
    } else if cfg.branch == cfg.buildkite_pipeline_default_branch {
        dest.push(cfg.branch.clone());
        dest.push(cfg.commit.clone());
    } else {
        dest.push(cfg.buildkite_job_id.clone());
    }

    format!("gs://{}", dest.to_str().expect("Invalid upload path"))
}
