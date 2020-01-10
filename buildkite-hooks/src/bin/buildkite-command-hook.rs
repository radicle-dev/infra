use std::fmt;
use std::io;
use std::iter;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;
use std::time::Duration;

use failure::Fail;
use log::{debug, info};
use paw;

use buildkite_hooks::config::Config;
use buildkite_hooks::container::docker::*;
use buildkite_hooks::env;
use buildkite_hooks::timeout::Timeout;

#[derive(Debug, Fail)]
enum Error {
    #[fail(display = "Child process `{}` was killed by a signal", 0)]
    ChildKilled(DisplayCommand),

    #[fail(display = "Child process `{}` was killed by a signal", 0)]
    ChildKilledAny(String),

    #[fail(display = "Invalid container registry for image: {}", 0)]
    InvalidImageRegistry(String),

    #[fail(display = "No Dockerfile given")]
    NoDockerFile,

    #[fail(display = "No image name given")]
    NoImageName,

    #[fail(display = "{}", 0)]
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

// TODO: should generalise this hackery
struct CompletedCommand {
    cmd: Command,
    status: process::ExitStatus,
}

impl From<CompletedCommand> for Result<(), Error> {
    fn from(compl: CompletedCommand) -> Self {
        if compl.status.success() {
            Ok(())
        } else {
            compl
                .status
                .code()
                .map_or(Err(Error::ChildKilled(compl.cmd.into())), |code| {
                    Err(Error::Io(io::Error::from_raw_os_error(code)))
                })
        }
    }
}

#[derive(Debug)]
struct DisplayCommand(String);

impl From<Command> for DisplayCommand {
    fn from(cmd: Command) -> Self {
        DisplayCommand(format!("{:?}", cmd))
    }
}

impl fmt::Display for DisplayCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

struct VolumeMounts {
    build_cache: Mount,
    img_cache: Mount,
    sources: Mount,
    tmpfs: Mount,
}

#[paw::main]
fn main(cfg: Config) -> Result<(), Error> {
    env_logger::init();

    let cfg = cfg.valid();

    let docker = Docker::new();
    let timeout = Timeout::new(Duration::from_secs(cfg.timeout_minutes as u64 * 60));

    // Setup cache volumes
    let mounts = setup_volumes(&docker, &cfg)?;

    // Unlock secrets
    info!("Decrypting secrets");
    decrypt_repo_secrets(&cfg)?;

    // Pull or build container image
    let mut build_container_image = match cfg.build_container_image {
        Some(ref img) => img.clone(),
        None => format!(
            "gcr.io/opensourcecoin/{}-build:{}",
            cfg.buildkite_pipeline_slug, cfg.commit
        ),
    };

    if !build_container_image.starts_with("gcr.io/opensourcecoin/") {
        return Err(Error::InvalidImageRegistry(build_container_image));
    }

    info!("Pulling docker image {}", build_container_image);
    let cfg2 = cfg.clone(); // prevent move into closure
    docker.pull(&build_container_image).or_else(|_| {
        // Re-tag with current commit
        let colon = ':';
        build_container_image = format!(
            "{}:{}",
            build_container_image
                .chars()
                .take_while(|chr| chr != &colon)
                .collect::<String>(),
            cfg2.commit,
        );

        info!(
            "Couldn't pull image, building instead as {}",
            build_container_image
        );
        cfg2.clone()
            .build_container_dockerfile
            .map_or(Err(Error::NoDockerFile), |dockerfile| {
                build_image(
                    &docker,
                    &cfg2,
                    &timeout,
                    &mounts.img_cache,
                    &build_container_image,
                    &dockerfile,
                )
            })
    })?;

    // Run build command
    info!("Running build command");
    docker.run_build(
        RunBuildOptions {
            build_id: cfg.command_id(),
            image: build_container_image,
            cmd: cfg.build_command.clone(),
            mounts: vec![mounts.sources, mounts.tmpfs, mounts.build_cache],
            env: env::safe_buildkite_vars().chain(env::build_vars()),
            runtime: if cfg.is_trusted_build() {
                Runtime::Runc
            } else {
                Runtime::Kata
            },
        },
        &timeout,
    )?;

    // Build step container image
    match (&cfg.step_container_dockerfile, &cfg.step_container_image) {
        (Some(ref dockerfile), Some(ref image_name)) => {
            info!("Building step container image");
            build_image(
                &docker,
                &cfg,
                &timeout,
                &mounts.img_cache,
                &image_name,
                &dockerfile,
            )
        }

        (None, Some(_)) => Err(Error::NoDockerFile),
        (Some(_), None) => Err(Error::NoImageName),
        (None, None) => Ok(()),
    }
}

fn decrypt_repo_secrets(cfg: &Config) -> Result<(), Error> {
    let secrets_yaml = cfg.checkout_path.join(".buildkite/secrets.yaml");
    if secrets_yaml.exists() {
        let mut cmd = Command::new("sops");
        cmd.args(&[
            "--output-type",
            "dotenv",
            "--output",
            ".secrets",
            "--decrypt",
        ])
        .arg(secrets_yaml);
        cmd.status()
            .map_err(|e| e.into())
            .and_then(|status| CompletedCommand { cmd, status }.into())
    } else {
        debug!("No .buildkite/secrets.yaml in repository");
        Ok(())
    }
}

fn setup_volumes<C>(contained: &C, cfg: &Config) -> Result<VolumeMounts, Error>
where
    C: Containeriser,
{
    let cache_volume_prefix = format!(
        "cache_{}_{}_{}",
        cfg.buildkite_agent_name, cfg.buildkite_organization_slug, cfg.buildkite_pipeline_slug,
    );

    let master_cache_volume_name = format!(
        "{}_{}",
        cache_volume_prefix, cfg.buildkite_pipeline_default_branch
    );

    let default_volume_opts = vec![
        ("quota".into(), format!("{}GiB", cfg.build_cache_quota_gib)),
        ("exec".into(), "on".into()),
    ];

    let master_cache_volume = contained.create_volume(CreateVolumeOptions {
        name: master_cache_volume_name.clone(),
        driver: Some("zockervols".into()),
        volume_opts: default_volume_opts.clone(),
        labels: vec![],
    })?;

    let cache_volume_mount = if cfg.is_trusted_build() {
        let cache_volume = if cfg.branch == cfg.buildkite_pipeline_default_branch {
            Ok(master_cache_volume)
        } else {
            contained.create_volume(CreateVolumeOptions {
                name: format!("{}_{}", cache_volume_prefix, cfg.branch),
                driver: Some("zockervols".into()),
                volume_opts: iter::once(("from".into(), master_cache_volume_name))
                    .chain(default_volume_opts.iter().cloned())
                    .collect(),
                labels: vec!["build-cache".into()],
            })
        }?;

        Mount::Volume {
            src: Some(cache_volume),
            dst: PathBuf::from("/cache"),
            readonly: false,
            volume_driver: Some("zockervols".into()),
            volume_opts: vec![],
        }
    } else {
        Mount::Volume {
            src: None,
            dst: PathBuf::from("/cache"),
            readonly: false,
            volume_driver: Some("zockervols".into()),
            volume_opts: default_volume_opts,
        }
    };

    let img_cache_volume_name = format!(
        "img_{}_{}_{}",
        cfg.buildkite_agent_name, cfg.buildkite_organization_slug, cfg.buildkite_pipeline_slug
    );

    let img_cache_volume = contained.create_volume(CreateVolumeOptions {
        name: img_cache_volume_name.clone(),
        driver: Some("zockervols".into()),
        volume_opts: vec![
            ("exec".into(), "on".into()),
            ("setuid".into(), "on".into()),
            ("quota".into(), format!("{}GiB", cfg.img_cache_quota_gib)),
        ],
        labels: vec!["build_cache".into()],
    })?;

    let img_cache_volume_mount = if cfg.is_trusted_build() {
        Mount::Volume {
            src: Some(img_cache_volume),
            dst: PathBuf::from("/cache"),
            readonly: false,
            volume_driver: None,
            volume_opts: vec![],
        }
    } else {
        Mount::Volume {
            src: Some(img_cache_volume),
            dst: PathBuf::from("/cache"),
            readonly: false,
            volume_driver: Some("zockervols".into()),
            volume_opts: vec![("from".into(), img_cache_volume_name)],
        }
    };

    Ok(VolumeMounts {
        build_cache: cache_volume_mount,
        img_cache: img_cache_volume_mount,
        sources: Mount::Bind {
            src: cfg.checkout_path.clone(),
            dst: PathBuf::from("/build"),
            readonly: false,
        },
        tmpfs: Mount::Tmpfs {
            dst: PathBuf::from("/tmp"),
            size_in_bytes: cfg.tmp_size_bytes,
            mode: 0o777,
        },
    })
}

fn build_image<C>(
    contained: &C,
    cfg: &Config,
    timeout: &Timeout,
    cache: &Mount,
    image_name: &str,
    dockerfile: &Path,
) -> Result<(), Error>
where
    C: Containeriser,
{
    let status = contained.build_image(
        BuildImageOptions {
            build_id: cfg.command_id(),
            image: image_name.to_string(),
            dockerfile: dockerfile.to_path_buf(),
            context: dockerfile.parent().unwrap_or(&Path::new(".")).to_path_buf(),
            sources: cfg.checkout_path.clone(),
            cache: cache.clone(),
            build_args: env::safe_buildkite_vars(),
        },
        timeout,
    )?;

    if status.success() {
        Ok(())
    } else {
        status
            .code()
            .map_or(Err(Error::ChildKilledAny("build_image".into())), |code| {
                Err(Error::Io(io::Error::from_raw_os_error(code)))
            })
    }
}
