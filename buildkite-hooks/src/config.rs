use std::ffi::OsStr;
use std::fmt;
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;

use failure::Fail;
use structopt::StructOpt;
use url;
use url::Url;
use users::{get_group_by_name, get_user_by_name, Group, User};

use crate::container::VolumeDriver;

pub const MIN_TIMEOUT_MINUTES: u8 = 50;
pub const MAX_TIMEOUT_MINUTES: u8 = 240;
pub const MAX_BUILD_CACHE_QUOTA_GIB: u8 = 50;
pub const MAX_IMG_CACHE_QUOTA_GIB: u8 = 50;
pub const MAX_TMP_SIZE_BYTES: u32 = 500_000_000;

#[derive(Clone, Debug, StructOpt)]
pub struct Config {
    /// Comma-separated list of GitHub organisations considered "trusted"
    #[structopt(long, default_value = "monadic-xyz,oscoin,radicle-dev")]
    pub trusted_github_orgs: CommaSepVec,

    /// Build timeout in minutes
    #[structopt(long, default_value = "50")]
    pub timeout_minutes: u8,

    /// Quota for build cache volumes, in GiB
    #[structopt(long, default_value = "8")]
    pub build_cache_quota_gib: u8,

    /// Quota for image cache volumes, in GiB
    #[structopt(long, default_value = "20")]
    pub img_cache_quota_gib: u8,

    /// Size in bytes of the tmpfs mount for build containers
    #[structopt(long, default_value = "200000000")]
    pub tmp_size_bytes: u32,

    /// The username to drop privileges to for build containers
    #[structopt(long, default_value = "buildkite-builder", parse(try_from_str = getpwnam))]
    pub builder_user: User,

    /// The groupname to drop privileges to for build containers
    #[structopt(long, default_value = "buildkite-builder", parse(try_from_str = getgrnam))]
    pub builder_group: Group,

    /// Path to the GCP service account credentials file.
    ///
    /// Empty value or the string "instance" to use instance credentials.
    #[structopt(
        long,
        default_value = "/etc/gce/cred.json",
        env = "GOOGLE_APPLICATION_CREDENTIALS",
        parse(from_os_str)
    )]
    pub google_application_credentials: GoogleApplicationCredentials,

    /// Set the Docker volume driver.
    ///
    /// This is provided for local testing if ZFS is not available.
    #[structopt(long, default_value = "zockervols")]
    pub volume_driver: VolumeDriver,

    /// The docker image to use for running the build command
    #[structopt(long, env = "DOCKER_IMAGE")]
    pub build_container_image: Option<String>,

    /// Path to the Dockerfile (relative to the source repo) to use for building the
    /// build-container-image on CI
    #[structopt(long, env = "DOCKER_FILE", parse(from_os_str))]
    pub build_container_dockerfile: Option<PathBuf>,

    /// The fully-qualified name of a docker image to build as part of a build step
    #[structopt(long, env = "STEP_DOCKER_IMAGE")]
    pub step_container_image: Option<String>,

    /// Path to the Dockerfile (relative to the source repo) to use for building the
    /// step-container-image
    #[structopt(long, env = "STEP_DOCKER_FILE", parse(from_os_str))]
    pub step_container_dockerfile: Option<PathBuf>,

    /// Path to the directory (relative to the source repo) to use as the build context for
    /// step-container-image
    #[structopt(long, env = "STEP_DOCKER_CONTEXT", parse(from_os_str))]
    pub step_container_context: Option<PathBuf>,

    /// Whether to use a shared master cache (i.e. if true, do not scope the
    /// cache volume to the agent instance; if false maintain a separate cache
    /// per agent).
    ///
    /// It is the users responsibility to constrain the concurrency of master
    /// builds via the pipeline settings.
    #[structopt(
        long,
        default_value = "false",
        env = "SHARED_MASTER_CACHE",
        parse(try_from_str)
    )]
    pub shared_master_cache: bool,

    /// The branch being built
    #[structopt(long, env = "BUILDKITE_BRANCH")]
    pub branch: String,

    /// The commit being built
    #[structopt(long, env = "BUILDKITE_COMMIT")]
    pub commit: String,

    /// The build command
    #[structopt(long, env = "BUILDKITE_COMMAND")]
    pub build_command: String,

    /// The upstream repo
    #[structopt(long, env = "BUILDKITE_REPO")]
    pub upstream_repo: Url,

    /// The PR repo, if any
    #[structopt(long, env = "BUILDKITE_PULL_REQUEST_REPO")]
    pub pull_request_repo: MaybeEmpty<Url>,

    /// The build ID
    #[structopt(long, env = "BUILDKITE_BUILD_ID")]
    pub build_id: String,

    /// The build step ID
    #[structopt(long, env = "BUILDKITE_STEP_ID")]
    pub step_id: String,

    /// The path where Buildkite will check out the sources for this build to
    #[structopt(long, env = "BUILDKITE_BUILD_CHECKOUT_PATH")]
    pub checkout_path: PathBuf,

    #[structopt(long, env = "BUILDKITE_AGENT_NAME")]
    pub buildkite_agent_name: String,

    #[structopt(long, env = "BUILDKITE_ORGANIZATION_SLUG")]
    pub buildkite_organization_slug: String,

    #[structopt(long, env = "BUILDKITE_PIPELINE_SLUG")]
    pub buildkite_pipeline_slug: String,

    #[structopt(long, env = "BUILDKITE_PIPELINE_DEFAULT_BRANCH")]
    pub buildkite_pipeline_default_branch: String,
}

impl Config {
    pub fn valid(mut self) -> Self {
        self.timeout_minutes = match self.timeout_minutes {
            timeout if timeout > MAX_TIMEOUT_MINUTES => MAX_TIMEOUT_MINUTES,
            timeout if timeout < MIN_TIMEOUT_MINUTES => MIN_TIMEOUT_MINUTES,
            timeout => timeout,
        };

        self.build_cache_quota_gib = if self.build_cache_quota_gib > MAX_BUILD_CACHE_QUOTA_GIB {
            MAX_BUILD_CACHE_QUOTA_GIB
        } else {
            self.build_cache_quota_gib
        };

        self.img_cache_quota_gib = if self.img_cache_quota_gib > MAX_IMG_CACHE_QUOTA_GIB {
            MAX_IMG_CACHE_QUOTA_GIB
        } else {
            self.img_cache_quota_gib
        };

        self.tmp_size_bytes = if self.tmp_size_bytes > MAX_TMP_SIZE_BYTES {
            MAX_TMP_SIZE_BYTES
        } else {
            self.tmp_size_bytes
        };

        // For some reason, the command hook gets invoked for uploading the repo
        // pipeline.yaml (`buildkite-agent pipeline upload`), too. Make sure we
        // run this in a vanilla alpine containers, and disable all image
        // building.
        if self.is_agent_command() {
            self.build_container_image = Some("alpine".into());
            self.build_container_dockerfile = None;
            self.step_container_image = None;
            self.step_container_dockerfile = None;
        }

        self
    }

    /// A unique ID per `command` hook invocation
    pub fn command_id(&self) -> String {
        format!("{}-{}", self.build_id, self.step_id)
    }

    pub fn is_agent_command(&self) -> bool {
        self.build_command.starts_with("buildkite-agent")
    }

    pub fn is_trusted_build(&self) -> bool {
        let trusted_orgs = &self.trusted_github_orgs.0;
        if is_trusted_github_url(&self.upstream_repo, &trusted_orgs) {
            match &self.pull_request_repo.deref() {
                None => true,
                Some(url) => is_trusted_github_url(&url, &trusted_orgs),
            }
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct CommaSepVec(Vec<String>);

impl FromStr for CommaSepVec {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(CommaSepVec(s.split(',').map(|x| x.to_string()).collect()))
    }
}

impl fmt::Debug for CommaSepVec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone)]
pub struct MaybeEmpty<T>(Option<T>);

impl<T: FromStr> FromStr for MaybeEmpty<T> {
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(MaybeEmpty(None))
        } else {
            T::from_str(s).map(Some).map(MaybeEmpty)
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for MaybeEmpty<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> Deref for MaybeEmpty<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn is_trusted_github_url(url: &Url, trusted_orgs: &[String]) -> bool {
    if url.host_str() == Some("github.com") {
        if let Some(Some(org)) = url.path_segments().map(|mut ps| ps.nth(0)) {
            trusted_orgs.iter().any(|x| x == org)
        } else {
            false
        }
    } else {
        false
    }
}

#[derive(Debug, Fail)]
enum IdMapError {
    #[fail(display = "No such user {}", 0)]
    NoSuchUser(String),

    #[fail(display = "No such group {}", 0)]
    NoSuchGroup(String),
}

fn getpwnam(username: &str) -> Result<User, IdMapError> {
    get_user_by_name(username).ok_or_else(|| IdMapError::NoSuchUser(username.to_string()))
}

fn getgrnam(groupname: &str) -> Result<Group, IdMapError> {
    get_group_by_name(groupname).ok_or_else(|| IdMapError::NoSuchGroup(groupname.to_string()))
}

#[derive(Clone, Debug)]
pub enum GoogleApplicationCredentials {
    Instance,
    Json(PathBuf),
}

impl From<&OsStr> for GoogleApplicationCredentials {
    fn from(s: &OsStr) -> Self {
        if s.is_empty() || s == "instance" {
            Self::Instance
        } else {
            Self::Json(PathBuf::from(s))
        }
    }
}
