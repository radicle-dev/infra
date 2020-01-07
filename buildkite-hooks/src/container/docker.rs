use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use libc;

use crate::cmd::CommandExt;
pub use crate::container::*;
use crate::timeout::Timeout;

pub const IMG_IMAGE: &str = "gcr.io/opensourcecoin/img@sha256:6a8661fc534f2341a42d6440e0c079aeaa701fe9d6c70b12280a1f8ce30b700c";

#[derive(Clone)]
pub struct Docker {}

impl Docker {
    pub fn new() -> Self {
        Docker {}
    }
}

impl Default for Docker {
    fn default() -> Self {
        Self::new()
    }
}

impl Containeriser for Docker {
    fn create_volume(&self, opts: CreateVolumeOptions) -> Result<Volume, io::Error> {
        let status = Command::new("docker")
            .arg("volume")
            .arg("create")
            .arg("--driver")
            .arg(opts.driver.clone().unwrap_or_else(|| "local".into()))
            .args(
                opts.volume_opts
                    .iter()
                    .map(|(k, v)| vec!["--opt".into(), format!("{}={}", k, v)])
                    .flatten(),
            )
            .args(
                opts.labels
                    .iter()
                    .map(|label| vec!["--label", label])
                    .flatten(),
            )
            .status()?;

        if status.success() {
            Ok(Volume::Persistent {
                name: opts.name,
                driver: opts.driver,
                opts: opts.volume_opts,
                labels: opts.labels,
            })
        } else {
            status.code().map_or(
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "`docker volume create` killed by signal",
                )),
                |code| Err(io::Error::from_raw_os_error(code)),
            )
        }
    }

    fn run_build<Env, S>(
        &self,
        opts: RunBuildOptions<Env>,
        timeout: &Timeout,
    ) -> Result<ExitStatus, io::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>,
    {
        Command::new("docker")
            .arg("run")
            .arg("--tty")
            .arg("--rm")
            .args(&["--name", &format!("build-{}", opts.build_id)])
            .args(&["--label", &opts.build_id])
            .arg("--init")
            .arg("--read-only")
            .args(&[
                "--user",
                &format!("{}={}", get_effective_uid(), get_effective_gid()),
            ])
            .arg("--cap-drop=ALL")
            .arg("--security-opt=no-new-privileges")
            .args(&["--runtime".into(), opts.runtime.to_string()])
            .arg("--workdir=/build")
            .arg("--entrypoint=''")
            .args(opts.mounts.iter().map(render_mount_arg))
            .args(
                opts.env
                    .map(|(k, v)| vec!["--env".into(), format!("{}={}", k.as_ref(), v.as_ref())])
                    .flatten(),
            )
            .arg(opts.image)
            .args(&["/bin/sh", "-e", "-c", &opts.cmd])
            .safe_status(timeout.remaining())
    }

    fn build_image<Env, S>(
        &self,
        opts: BuildImageOptions<Env>,
        timeout: &Timeout,
    ) -> Result<ExitStatus, io::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>,
    {
        Command::new("docker")
            .arg("run")
            .arg("--tty")
            .arg("--rm")
            .args(&["--name", &format!("img-{}", opts.build_id)])
            .args(&["--label", &opts.build_id])
            .arg("--init")
            .args(&[
                "--security-opt=seccomp=unconfined",
                "--security-opt=apparmor=unconfined",
                "--security-opt=systempaths=unconfined",
                "--cap-drop=ALL",
                "--cap-add=SETUID",
                "--cap-add=SETGID",
            ])
            .args(
                [
                    Mount::Bind {
                        src: opts.sources.to_path_buf(),
                        dst: PathBuf::from("/build"),
                        readonly: true,
                    },
                    Mount::Bind {
                        src: PathBuf::from("/tmp"),
                        dst: PathBuf::from("/tar"),
                        readonly: false,
                    },
                    opts.cache.clone(),
                ]
                .iter()
                .map(render_mount_arg),
            )
            .arg(IMG_IMAGE)
            .arg("build")
            .args(
                opts.build_args
                    .map(|(k, v)| {
                        vec![
                            "--build-arg".into(),
                            format!("{}={}", k.as_ref(), v.as_ref()),
                        ]
                    })
                    .flatten(),
            )
            .args(&["--file", &opts.dockerfile.display().to_string()])
            .args(&["--tag", &opts.image])
            .arg("--no-console")
            .arg("--backend=native")
            .args(&["--state", &opts.cache.destination().display().to_string()])
            .args(&[
                "--output",
                &format!(
                    "type=docker,name={},dest=/tar/{}.tar",
                    opts.image, opts.build_id
                ),
            ])
            .arg(&opts.context.display().to_string())
            .safe_status(timeout.remaining())?;

        Command::new("docker")
            .arg("load")
            .arg("--quiet")
            .args(&["--input", &format!("/tmp/{}.tar", opts.build_id)])
            .safe_status(timeout.remaining())?;

        Command::new("docker")
            .arg("push")
            .arg(opts.image)
            .safe_status(timeout.remaining())
    }

    fn reap_containers(&self, build_id: &str) -> Result<ExitStatus, io::Error> {
        let ps_out = Command::new("docker")
            .args(&[
                "ps",
                "--filter",
                &format!("label={}", build_id),
                "--format",
                "{{.ID}}",
            ])
            .output()?;

        if !ps_out.status.success() {
            return Ok(ps_out.status);
        }

        let nl: u8 = 10;
        ps_out
            .stdout
            .split(|x| x == &nl)
            .map(OsStr::from_bytes)
            .for_each(|container| {
                let _ = Command::new("docker")
                    .args(&[OsStr::new("kill"), container])
                    .status();
                let _ = Command::new("docker")
                    .args(&[OsStr::new("rm"), container])
                    .status();
            });

        Ok(ExitStatus::from_raw(0))
    }

    fn pull(&self, image: &str) -> Result<(), io::Error> {
        let status = Command::new("docker").arg("pull").arg(image).status()?;
        if status.success() {
            Ok(())
        } else {
            status.code().map_or(
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    "`docker pull` killed by signal",
                )),
                |code| Err(io::Error::from_raw_os_error(code)),
            )
        }
    }
}

fn render_mount_arg(mount: &Mount) -> String {
    match mount {
        Mount::Tmpfs {
            dst,
            size_in_bytes,
            mode,
        } => format!(
            "--mount=type=tmpfs,dst={},tmpfs-size={},tmpfs-mode={:o}",
            dst.display(),
            size_in_bytes,
            mode
        ),
        Mount::Bind { src, dst, readonly } => {
            let mut arg = format!(
                "--mount=type=bind,src={},dst={}",
                src.display(),
                dst.display()
            );
            if *readonly {
                arg.push_str(",readonly");
            }
            arg
        }
        Mount::Volume {
            src,
            dst,
            readonly,
            volume_driver,
            volume_opts,
        } => {
            let mut arg = format!("--mount=type=volume,dst={}", dst.display());

            if let Some(Volume::Persistent { name, .. }) = src {
                arg.push_str(&format!(",src={}", name));
            }

            if *readonly {
                arg.push_str(",readonly");
            }

            if let Some(driver) = volume_driver {
                arg.push_str(",volume-driver=");
                arg.push_str(driver);
            }

            for (k, v) in volume_opts {
                arg.push_str(&format!(",volume-opt={}={}", k, v));
            }

            arg
        }
    }
}

fn get_effective_uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn get_effective_gid() -> u32 {
    unsafe { libc::getegid() }
}
