use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Command;

use crate::cmd;
use crate::cmd::CommandExt;
pub use crate::container::*;
use crate::timeout::Timeout;

pub const IMG_IMAGE: &str = "gcr.io/opensourcecoin/img@sha256:6a8661fc534f2341a42d6440e0c079aeaa701fe9d6c70b12280a1f8ce30b700c";

#[derive(Clone)]
pub struct Docker {
    build_id: String,
}

impl Docker {
    pub fn new(build_id: &str) -> Self {
        Docker {
            build_id: build_id.into(),
        }
    }

    fn cmd(&self) -> Command {
        Command::new("docker")
    }
}

impl Drop for Docker {
    fn drop(&mut self) {
        self.reap_containers()
            .unwrap_or_else(|e| eprintln!("Error reaping containers (in Drop): {}", e))
    }
}

impl Containeriser for Docker {
    fn create_volume(&self, opts: CreateVolumeOptions) -> Result<Volume, cmd::Error> {
        self.cmd()
            .arg("volume")
            .arg("create")
            .arg("--driver")
            .arg(
                opts.driver
                    .clone()
                    .unwrap_or_else(|| VolumeDriver::Local)
                    .to_string(),
            )
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
            .safe()?
            .succeed()
            .map(|()| Volume::Persistent {
                name: opts.name,
                driver: opts.driver,
                opts: opts.volume_opts,
                labels: opts.labels,
            })
    }

    fn run_build<Env, S>(
        &self,
        opts: RunBuildOptions<Env>,
        timeout: &Timeout,
    ) -> Result<(), cmd::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>,
    {
        self.cmd()
            .arg("run")
            .arg("--tty")
            .arg("--rm")
            .args(&["--name", &format!("build-{}", &self.build_id)])
            .args(&["--label", &self.build_id])
            .arg("--init")
            .arg("--read-only")
            .args(&["--user", &format!("{}={}", opts.uid, opts.gid)])
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
            .safe()?
            .timeout(timeout.remaining())
            .succeed()
    }

    fn build_image<Env, S>(
        &self,
        opts: BuildImageOptions<Env>,
        timeout: &Timeout,
    ) -> Result<(), cmd::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>,
    {
        self.cmd()
            .arg("run")
            .arg("--tty")
            .arg("--rm")
            .args(&["--name", &format!("img-{}", self.build_id)])
            .args(&["--label", &self.build_id])
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
                    opts.image, self.build_id
                ),
            ])
            .arg(&opts.context.display().to_string())
            .safe()?
            .timeout(timeout.remaining())
            .succeed()?;

        self.cmd()
            .arg("load")
            .arg("--quiet")
            .args(&["--input", &format!("/tmp/{}.tar", self.build_id)])
            .safe()?
            .timeout(timeout.remaining())
            .succeed()?;

        self.cmd()
            .arg("push")
            .arg(opts.image)
            .safe()?
            .timeout(timeout.remaining())
            .succeed()
    }

    fn reap_containers(&self) -> Result<(), cmd::Error> {
        let mut ps = self.cmd();
        ps.args(&[
            "ps",
            "--filter",
            &format!("label={}", self.build_id),
            "--format",
            "{{.ID}}",
        ]);
        let out = ps
            .output()
            .map_err(|e| cmd::Error::Io(cmd::command_line(&ps), e))?;

        if !out.status.success() {
            return Err(cmd::Error::NonZeroExitStatus(
                cmd::command_line(&ps),
                out.status,
            ));
        }

        let nl: u8 = 10;
        out.stdout
            .split(|x| x == &nl)
            .map(OsStr::from_bytes)
            .for_each(|container| {
                let _ = self.cmd().args(&[OsStr::new("kill"), container]).status();
                let _ = self.cmd().args(&[OsStr::new("rm"), container]).status();
            });

        Ok(())
    }

    fn pull(&self, image: &str) -> Result<(), cmd::Error> {
        self.cmd().arg("pull").arg(image).safe()?.succeed()
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
                arg.push_str(&driver.to_string());
            }

            for (k, v) in volume_opts {
                arg.push_str(&format!(",volume-opt={}={}", k, v));
            }

            arg
        }
    }
}
