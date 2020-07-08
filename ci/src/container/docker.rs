use std::{path::PathBuf, process::Command};

pub use crate::container::*;
use crate::{cmd, cmd::CommandExt, timeout::Timeout};

pub const IMG_IMAGE: &str = "gcr.io/opensourcecoin/img@sha256:\
                             24252f659024808246d8c4d674f19d8d923688cd5f857f4a607fe8dbf42c491c";

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

    fn cmd(&self) -> Command { Command::new("docker") }
}

impl Drop for Docker {
    fn drop(&mut self) {
        self.reap_containers()
            .unwrap_or_else(|e| eprintln!("Error reaping containers (in Drop): {}", e))
    }
}

impl Containeriser for Docker {
    fn create_volume(&self, opts: CreateVolumeOptions) -> Result<Volume, cmd::Error> {
        log::debug!("Docker::create_volume({:?})", opts);

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
            .arg(&opts.name)
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
        let mut docker = self.cmd();

        docker
            .arg("run")
            .arg("--tty")
            .arg("--rm")
            .args(&["--name", &format!("build-{}", &self.build_id)])
            .args(&["--label", &self.build_id])
            .arg("--read-only")
            .args(&["--user", &format!("{}:{}", opts.uid, opts.gid)])
            .arg("--cap-drop=ALL")
            .arg("--security-opt=no-new-privileges")
            .args(&["--runtime".into(), opts.runtime.to_string()])
            .arg("--workdir=/build")
            .arg("--entrypoint=")
            .args(opts.mounts.iter().map(render_mount_arg))
            .args(
                opts.env
                    .map(|(k, v)| vec!["--env".into(), format!("{}={}", k.as_ref(), v.as_ref())])
                    .flatten(),
            );

        // Don't use a PID1 with kata due to
        // https://github.com/kata-containers/runtime/issues/1901
        //
        // The main reason we want a PID1 is to be able to interrupt builds by
        // sending a signal to the docker _client_ (assuming the PID1 forwards
        // it properly). We hope for now that the [`Drop`] impl for [`Docker`]
        // will solve this anyway. Otherwise we'll need to jump through some
        // hoops to provide a CoW'ed tini to kata containers.
        if opts.runtime != Runtime::Kata {
            docker.arg("--init");
        }

        docker
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
            .arg("--env=IMG_DISABLE_EMBEDDED_RUNC=1")
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
        log::debug!("Removing containers for build {}", self.build_id);
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

        let stdout = String::from_utf8_lossy(&out.stdout);
        let containers = stdout.split_terminator(&"\n");

        for container in containers {
            let mut cmd = self.cmd();
            cmd.args(&["rm", "--force", container]);
            let result = cmd.status();
            match result {
                Err(err) => log::error!("Running \"{:?}\" failed: {}", cmd, err),
                Ok(exit_status) => {
                    if !exit_status.success() {
                        log::error!("Command \"{:?}\" exited with status {}", cmd, exit_status,)
                    }
                },
            }
        }

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
            "--tmpfs={}:size={},mode={:o},exec",
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
        },
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
                arg.push_str(&format!(",volume-driver={}", driver));
            }

            for (k, v) in volume_opts {
                arg.push_str(&format!(",volume-opt={}={}", k, v));
            }

            // Prevent existing data at the image mountpoint to mess up
            // permissions. See #41
            arg.push_str(",volume-nocopy=true");

            arg
        },
    }
}
