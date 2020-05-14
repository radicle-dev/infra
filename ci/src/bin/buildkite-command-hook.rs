use std::{
    iter,
    path::{Path, PathBuf},
    time::Duration,
};

use failure::{format_err, Error};
use log::info;
use paw;

use buildkite_hooks::{cmd, config::Config, container::docker::*, env, timeout::Timeout};

struct VolumeMounts {
    build_cache: Mount,
    img_cache: Mount,
    sources: Mount,
    tmpfs: Mount,
    buildkite_agent: Mount,
}

#[paw::main]

fn main(cfg: Config) {
    env_logger::init();
    if let Err(err) = main_(cfg) {
        log::error!("{}", err);
        std::process::exit(1)
    }
}

fn main_(cfg: Config) -> Result<(), Error> {
    let cfg = cfg.valid();

    let docker = Docker::new(&cfg.command_id());

    let timeout = Timeout::new(Duration::from_secs(cfg.timeout_minutes as u64 * 60));

    // Setup cache volumes
    let mounts = setup_volumes(&docker, &cfg)?;

    // Pull or build container image
    let mut build_container_image = {
        let image = match cfg.build_container_image {
            Some(ref img) => img.clone(),
            None => format!(
                "gcr.io/opensourcecoin/{}-build:{}",
                cfg.buildkite_pipeline_slug, cfg.commit
            ),
        };

        if cfg.is_agent_command() || image.starts_with("gcr.io/opensourcecoin/") {
            Ok(image)
        } else {
            Err(format_err!("Invalid image registry {}", image))
        }
    }?;

    info!("Pulling docker image {}", build_container_image);

    let cfg2 = cfg.clone(); // prevent move into closure
    docker.pull(&build_container_image).or_else(|e| {
        info!("Failed to pull image {}: {}", build_container_image, e);

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

        cfg2.clone().build_container_dockerfile.map_or(
            Err(format_err!(
                "No Dockerfile given to build the build container with"
            )),
            |dockerfile| {
                info!("Building build container image {}", build_container_image);

                build_image(
                    &docker,
                    &cfg2,
                    &timeout,
                    &mounts.img_cache,
                    &build_container_image,
                    &dockerfile,
                )
            },
        )
    })?;

    // Run build command
    info!("Running build command");

    docker
        .run_build(
            RunBuildOptions {
                image: build_container_image,
                cmd: cfg.build_command.clone(),
                mounts: vec![
                    mounts.sources,
                    mounts.tmpfs,
                    mounts.build_cache,
                    mounts.buildkite_agent,
                ],
                env: env::safe_buildkite_vars().chain(env::build_vars()),
                runtime: if cfg.is_trusted_build() {
                    Runtime::Runc
                } else {
                    Runtime::Kata
                },
                uid: cfg.builder_user.uid(),
                gid: cfg.builder_group.gid(),
            },
            &timeout,
        )
        .map_err(|err| match err {
            cmd::Error::NonZeroExitStatus(_, status) => {
                format_err!("Build command exited with {}", status)
            },
            err => err.into(),
        })?;

    // Build step container image
    match (&cfg.step_container_dockerfile, &cfg.step_container_image) {
        (Some(ref dockerfile), Some(ref image_name)) => {
            info!("Building step container image {}", image_name);

            build_image(
                &docker,
                &cfg,
                &timeout,
                &mounts.img_cache,
                &image_name,
                &dockerfile,
            )
        },

        (None, Some(image_name)) => Err(format_err!(
            "No Dockerfile given to build {} with",
            image_name
        )),
        (Some(dockerfile), None) => Err(format_err!(
            "No image name given to build using Dockerfile {:?}",
            dockerfile
        )),
        (None, None) => Ok(()),
    }
}

fn setup_volumes<C>(contained: &C, cfg: &Config) -> Result<VolumeMounts, Error>
where
    C: Containeriser,
{
    let volume_driver = Some(cfg.volume_driver.clone());

    let cache_volume_prefix = format!(
        "cache_{}_{}_{}",
        if cfg.shared_master_cache {
            "shared"
        } else {
            &cfg.buildkite_agent_name
        },
        cfg.buildkite_organization_slug,
        cfg.buildkite_pipeline_slug,
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
        driver: volume_driver.clone(),
        volume_opts: default_volume_opts.clone(),
        labels: vec![],
    })?;

    let cache_volume_mount = if cfg.is_trusted_build() {
        let cache_volume = if cfg.branch == cfg.buildkite_pipeline_default_branch {
            Ok(master_cache_volume)
        } else {
            contained.create_volume(CreateVolumeOptions {
                name: format!("{}_{}", cache_volume_prefix, cfg.branch),
                driver: volume_driver.clone(),
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
            volume_driver: volume_driver.clone(),
            volume_opts: vec![],
        }
    } else {
        Mount::Volume {
            src: None,
            dst: PathBuf::from("/cache"),
            readonly: false,
            volume_driver: volume_driver.clone(),
            volume_opts: default_volume_opts,
        }
    };

    let img_cache_volume_name = format!(
        "img_{}_{}_{}",
        cfg.buildkite_agent_name, cfg.buildkite_organization_slug, cfg.buildkite_pipeline_slug
    );

    let img_cache_volume = contained.create_volume(CreateVolumeOptions {
        name: img_cache_volume_name.clone(),
        driver: volume_driver.clone(),
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
            volume_driver,
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
        buildkite_agent: Mount::Bind {
            // TODO: should we check it's actually installed here?
            src: PathBuf::from("/usr/bin/buildkite-agent"),
            dst: PathBuf::from("/usr/bin/buildkite-agent"),
            readonly: true,
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
    contained
        .build_image(
            BuildImageOptions {
                image: image_name.to_string(),
                dockerfile: dockerfile.to_path_buf(),
                context: dockerfile.parent().unwrap_or(&Path::new(".")).to_path_buf(),
                sources: cfg.checkout_path.clone(),
                cache: cache.clone(),
                build_args: env::safe_buildkite_vars(),
            },
            timeout,
        )
        .map_err(|e| e.into())
}
