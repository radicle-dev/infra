use std::{fmt, path::PathBuf, str::FromStr};

use users::{gid_t, uid_t};

use crate::{cmd, timeout::Timeout};

pub mod docker;

#[derive(Clone, Debug)]

pub enum VolumeDriver {
    Local,
    Zockervols,
}

impl FromStr for VolumeDriver {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Self::Local),
            "zockervols" => Ok(Self::Zockervols),
            _ => Err(format!("Unsupported volume driver {}", s)),
        }
    }
}

impl fmt::Display for VolumeDriver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Local => f.write_str("local"),
            Self::Zockervols => f.write_str("zockervols"),
        }
    }
}

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
        volume_driver: Option<VolumeDriver>,
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

#[derive(Debug, PartialEq, Eq)]

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
        driver: Option<VolumeDriver>,
        opts: Vec<(String, String)>,
    },
    Persistent {
        name: String,
        driver: Option<VolumeDriver>,
        opts: Vec<(String, String)>,
        labels: Vec<String>,
    },
}

impl Volume {
    /// Create a new ephemeral volume

    pub fn new(driver: Option<VolumeDriver>, opts: Vec<(String, String)>) -> Self {
        Self::Ephemeral { driver, opts }
    }
}

#[derive(Debug)]
pub struct CreateVolumeOptions {
    pub name: String,
    pub driver: Option<VolumeDriver>,
    pub volume_opts: Vec<(String, String)>,
    pub labels: Vec<String>,
}

pub struct RunBuildOptions<Env> {
    pub image: String,
    pub cmd: String,
    pub mounts: Vec<Mount>,
    pub env: Env,
    pub runtime: Runtime,
    pub uid: uid_t,
    pub gid: gid_t,
}

pub struct BuildImageOptions<Env> {
    pub image: String,
    pub dockerfile: PathBuf,
    pub context: PathBuf,
    pub sources: PathBuf,
    pub cache: Mount,
    pub build_args: Env,
}

pub trait Containeriser {
    /// Create a persistent volume

    fn create_volume(&self, opts: CreateVolumeOptions) -> Result<Volume, cmd::Error>;

    /// Run the build command `cmd` in a container

    fn run_build<Env, S>(
        &self,
        opts: RunBuildOptions<Env>,
        timeout: &Timeout,
    ) -> Result<(), cmd::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>;

    /// Build a container image

    fn build_image<Env, S>(
        &self,
        opts: BuildImageOptions<Env>,
        timeout: &Timeout,
    ) -> Result<(), cmd::Error>
    where
        Env: Iterator<Item = (S, S)>,
        S: AsRef<str>;

    /// Reap any runaway containers

    fn reap_containers(&self) -> Result<(), cmd::Error>;

    /// Pull a container image

    fn pull(&self, image: &str) -> Result<(), cmd::Error>;
}
