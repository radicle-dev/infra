use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;

use crate::timeout::Timeout;

pub mod docker;

#[derive(Clone, Debug)]
pub enum Mount {
    Tmpfs {
        dst: PathBuf,
        size_in_bytes: u32,
        mode: u32,
    },
    Bind {
        src: PathBuf,
        dst: PathBuf,
        readonly: bool,
    },
    Volume {
        src: Option<Volume>,
        dst: PathBuf,
        readonly: bool,
        volume_driver: Option<String>,
        volume_opts: Vec<(String, String)>,
    },
}

impl Mount {
    pub fn destination(&self) -> PathBuf {
        match self {
            Self::Tmpfs { dst, .. } => dst.clone(),
            Self::Bind { dst, .. } => dst.clone(),
            Self::Volume { dst, .. } => dst.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Runtime {
    Runc,
    Kata,
}

impl fmt::Display for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Runc => f.write_str("runc"),
            Self::Kata => f.write_str("kata-containers"),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Volume {
    Ephemeral {
        driver: Option<String>,
        opts: Vec<(String, String)>,
    },
    Persistent {
        name: String,
        driver: Option<String>,
        opts: Vec<(String, String)>,
        labels: Vec<String>,
    },
}

impl Volume {
    /// Create a new ephemeral volume
    pub fn new(driver: Option<String>, opts: Vec<(String, String)>) -> Self {
        Self::Ephemeral { driver, opts }
    }
}

pub struct CreateVolumeOptions {
    pub name: String,
    pub driver: Option<String>,
    pub volume_opts: Vec<(String, String)>,
    pub labels: Vec<String>,
}

pub struct RunBuildOptions<Env> {
    pub build_id: String,
    pub image: String,
    pub cmd: String,
    pub mounts: Vec<Mount>,
    pub env: Env,
    pub runtime: Runtime,
}

pub struct BuildImageOptions<Env> {
    pub build_id: String,
    pub image: String,
    pub dockerfile: PathBuf,
    pub context: PathBuf,
    pub sources: PathBuf,
    pub cache: Mount,
    pub build_args: Env,
}

pub trait Containeriser {
    /// Create a persistent volume
    fn create_volume(&self, opts: CreateVolumeOptions) -> Result<Volume, io::Error>;

    /// Run the build command `cmd` in a container
    fn run_build<Env, S>(
        &self,
        opts: RunBuildOptions<Env>,
        timeout: &Timeout,
    ) -> Result<ExitStatus, io::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>;

    /// Build a container image
    fn build_image<Env, S>(
        &self,
        opts: BuildImageOptions<Env>,
        timeout: &Timeout,
    ) -> Result<ExitStatus, io::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>;

    /// Reap any runaway containers
    fn reap_containers(&self, build_id: &str) -> Result<ExitStatus, io::Error>;

    /// Pull a container image
    fn pull(&self, image: &str) -> Result<(), io::Error>;
}
