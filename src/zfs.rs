use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt;
use std::fmt::Display;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::SystemTime;

use byte_unit::Byte;
use chashmap::CHashMap;
use chrono::prelude::*;
use chrono::serde::ts_seconds;
use itertools::Itertools;
use regex::Regex;
use serde::Deserialize;
use users::{
    get_effective_gid, get_effective_groupname, get_effective_uid, get_effective_username,
};

use crate::api::*;

enum Cmd {
    Create { vol: String, opts: VolumeOptions },
    Destroy { vol: String },
    Mount { vol: String },
    Unmount { vol: String },
    List,
    GetMountpoint { vol: String },
    Inspect { vol: String },
}

impl Cmd {
    fn create(vol: &str, opts: VolumeOptions) -> Self {
        Cmd::Create {
            vol: sanitize_vol(vol),
            opts,
        }
    }

    fn destroy(vol: &str) -> Self {
        Cmd::Destroy {
            vol: sanitize_vol(vol),
        }
    }

    fn mount(vol: &str) -> Self {
        Cmd::Mount {
            vol: sanitize_vol(vol),
        }
    }

    fn unmount(vol: &str) -> Self {
        Cmd::Unmount {
            vol: sanitize_vol(vol),
        }
    }

    fn list() -> Self {
        Cmd::List
    }

    fn get_mountpoint(vol: &str) -> Self {
        Cmd::GetMountpoint {
            vol: sanitize_vol(vol),
        }
    }

    fn inspect(vol: &str) -> Self {
        Cmd::Inspect {
            vol: sanitize_vol(vol),
        }
    }

    fn run(&self, root: &PathBuf) -> Result<Vec<u8>, Error> {
        match self {
            Cmd::Create { vol, opts } => {
                let dataset = root.join(vol);

                match opts.snapshot_of {
                    Some(ref from) => {
                        // snapshot the `from` fs
                        let snap = format!(
                            "{}@{}",
                            root.join(sanitize_vol(from)).to_str().unwrap(),
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .expect("SystemTime before UNIX epoch!")
                                .as_nanos()
                        );
                        ZfsCmd::User.run(|zfs| zfs.arg("snapshot").arg(&snap))?;
                        // clone the snapshot as `vol`
                        ZfsCmd::User
                            .run(|zfs| {
                                zfs.arg("clone")
                                    .args(opts.as_args())
                                    .args(&["-o", "mountpoint=none"])
                                    .arg(snap.to_owned())
                                    .arg(&dataset)
                            })
                            .or_else(|e| ignore_already_exists(e).and(Ok(vec![])))
                            .or_else(|e| ignore_mount_error(e).and(Ok(vec![])))?;
                        // finally, mark the snapshot for deletion
                        ZfsCmd::User.run(|zfs| zfs.arg("destroy").arg("-d").arg(&snap))
                    }
                    None => ZfsCmd::User
                        .run(|zfs| {
                            zfs.arg("create")
                                .args(opts.as_args())
                                .args(&["-o", "mountpoint=none"])
                                .arg(&dataset)
                        })
                        .or_else(|e| ignore_mount_error(e).map(|_| vec![])),
                }?;

                // ZoL can't delegate mount permissions (via allow), but we
                // ultimately want the volume to be owned by the driver's user.
                // ZFS remembers the ownership / permissions if set on a mounted
                // dataset, though. So:
                //
                // We create the dataset without a mountpoint, so we don't need
                // root. Then, temporarily mount it and adjust the ownership and
                // permissions.
                let mountpoint = ZfsCmd::get_mountpoint_of(root)?.join(vol);

                ZfsCmd::set_mountpoint_of(&dataset, &mountpoint)?;

                let res1 = {
                    Command::new("sudo")
                        .arg("chown")
                        .arg({
                            // Try hard to use username:groupname instead of uid:gid
                            let mut user = get_effective_username()
                                .unwrap_or_else(|| get_effective_uid().to_string().into());
                            let group = get_effective_groupname()
                                .unwrap_or_else(|| get_effective_gid().to_string().into());

                            user.push(":");
                            user.push(group);
                            user
                        })
                        .arg(&mountpoint)
                        .run()?;

                    fs::set_permissions(&mountpoint, fs::Permissions::from_mode(0o750))
                };

                // Unmount in any case
                let res2 = ZfsCmd::remove_mountpoint_of(&dataset);

                match (res1, res2) {
                    (Err(e), _) => Err(e.into()),
                    (_, Err(e)) => Err(e),
                    (Ok(_), Ok(_)) => Ok(vec![]),
                }
            }

            Cmd::Destroy { vol } => {
                ZfsCmd::User.run(|zfs| zfs.args(&["destroy", "-r"]).arg(root.join(vol)))
            }

            Cmd::Mount { vol } => {
                let mountpoint = ZfsCmd::get_mountpoint_of(root)?.join(vol);
                ZfsCmd::set_mountpoint_of(&root.join(vol), &mountpoint)?;

                Ok(vec![])
            }

            Cmd::Unmount { vol } => ZfsCmd::remove_mountpoint_of(&root.join(vol)).map(|()| vec![]),

            Cmd::List => ZfsCmd::User.run(|zfs| {
                zfs.arg("list")
                    .args(&[
                        "-H",
                        "-p",
                        "-r",
                        "-o",
                        "name,mountpoint,creation,used,avail",
                    ])
                    .arg(root)
            }),

            Cmd::GetMountpoint { vol } => ZfsCmd::User.run(|zfs| {
                zfs.args(&["get", "mountpoint", "-H", "-o", "value"])
                    .arg(root.join(vol))
            }),

            Cmd::Inspect { vol } => ZfsCmd::User.run(|zfs| {
                zfs.arg("list")
                    .args(&["-H", "-p", "-o", "name,mountpoint,creation,used,avail"])
                    .arg(root.join(vol))
            }),
        }
    }
}

enum ZfsCmd {
    Sudo,
    User,
}

impl ZfsCmd {
    fn run<F>(&self, f: F) -> Result<Vec<u8>, Error>
    where
        F: FnOnce(&mut Command) -> &mut Command,
    {
        let sudo = self.sudo();
        let mut base = Command::new(if sudo { "sudo" } else { "zfs" });
        let cmd = {
            let cmd = if sudo {
                base.arg("zfs")
            } else {
                base.borrow_mut()
            };
            f(cmd)
        };
        cmd.run()
    }

    #[cfg(target_os = "linux")]
    fn sudo(&self) -> bool {
        if get_effective_uid() == 0 {
            false
        } else {
            match self {
                ZfsCmd::Sudo => true,
                ZfsCmd::User => false,
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn sudo(&self) -> bool {
        false
    }

    fn get_mountpoint_of(dataset: &Path) -> Result<PathBuf, Error> {
        Self::User
            .run(|zfs| {
                zfs.args(&["get", "mountpoint", "-H", "-o", "value"])
                    .arg(dataset)
            })
            .and_then(|stdout| {
                as_pathbuf(stdout)
                    .ok_or_else(|| Error::NoMountpointError(dataset.display().to_string()))
            })
    }

    fn set_mountpoint_of(dataset: &Path, mountpoint: &Path) -> Result<(), Error> {
        Self::Sudo
            .run(|zfs| {
                zfs.arg("set")
                    .arg(format!("mountpoint={}", mountpoint.display()))
                    .arg(dataset)
            })
            .map(|_| ())
    }

    fn remove_mountpoint_of(dataset: &Path) -> Result<(), Error> {
        Self::Sudo
            .run(|zfs| zfs.args(&["set", "mountpoint=none"]).arg(dataset))
            .map(|_| ())
    }
}

pub enum Error {
    IoError(io::Error),
    VolInUseError(String, Vec<String>),
    MountsLockError(String, String),
    CmdIoError(String, io::Error),
    CmdError(String, Vec<u8>),
    CmdOutputParseError(csv::Error),
    VolumeOptionsError(OptsError),
    NoMountpointError(String),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IoError(error)
    }
}

impl From<csv::Error> for Error {
    fn from(error: csv::Error) -> Self {
        Error::CmdOutputParseError(error)
    }
}

impl From<Error> for ErrorResponse {
    fn from(error: Error) -> Self {
        let err = match error {
            Error::IoError(e) => e.to_string(),
            Error::VolInUseError(vol, by) => {
                format!("Volume {} is in use by: {}", vol, by.into_iter().join(", "))
            }
            Error::MountsLockError(vol, e) => format!(
                "Could not acquire lock when trying to check mount status for volume {}: {}",
                vol, e
            ),
            Error::CmdIoError(cmd, e) => format!("{}: {}", cmd, e),
            Error::CmdError(cmd, stderr) => {
                format!("{}: {}", cmd, String::from_utf8_lossy(&stderr).into_owned())
            }
            Error::CmdOutputParseError(e) => e.to_string(),
            Error::VolumeOptionsError(e) => e.to_string(),
            Error::NoMountpointError(vol) => format!("No mountpoint for {}", vol),
        };
        ErrorResponse { err }
    }
}

pub struct VolumeOptions {
    snapshot_of: Option<String>,
    quota: u64,
    enable_compression: bool,
    enable_atime: bool,
    enable_exec: bool,
    enable_setuid: bool,
}

impl Default for VolumeOptions {
    fn default() -> Self {
        VolumeOptions {
            snapshot_of: None,
            quota: 1024 * 1024 * 250, // 250MiB
            enable_compression: true,
            enable_atime: false,
            enable_exec: false,
            enable_setuid: false,
        }
    }
}

impl VolumeOptions {
    fn as_properties<'a>(&self) -> HashMap<&'a str, String> {
        fn onoff(b: bool) -> String {
            if b { "on" } else { "off" }.to_string()
        }

        let mut props = HashMap::new();
        props.insert("quota", self.quota.to_string());
        props.insert("compression", onoff(self.enable_compression));
        props.insert("atime", onoff(self.enable_atime));
        props.insert("exec", onoff(self.enable_exec));
        props.insert("setuid", onoff(self.enable_setuid));

        props
    }

    fn as_args(&self) -> Vec<String> {
        vec!["-o".to_string()]
            .into_iter()
            .chain(
                self.as_properties()
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .intersperse("-o".to_string()),
            )
            .collect()
    }
}

pub struct OptsError(&'static str);

impl Display for OptsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<OptsError> for Error {
    fn from(error: OptsError) -> Self {
        Error::VolumeOptionsError(error)
    }
}

// `TryFrom` is used for parsing user options
impl TryFrom<HashMap<String, String>> for VolumeOptions {
    type Error = OptsError;

    fn try_from(opts: HashMap<String, String>) -> Result<Self, Self::Error> {
        let def = VolumeOptions::default();

        let quota = match opts.get("quota") {
            Some(x) => Byte::from_str(x)
                .map_err(|_| OptsError("Invalid quota specified"))
                .and_then(|byte| {
                    u64::try_from(byte.get_bytes()).map_err(|_| OptsError("Quota out of range"))
                }),
            None => Ok(def.quota),
        }?;

        fn option_enabled(opts: &HashMap<String, String>, opt: &str, def: bool) -> bool {
            opts.get(opt).map(|x| x == "on").unwrap_or(def)
        }

        Ok(VolumeOptions {
            snapshot_of: opts
                .get("snapshot-of")
                .or_else(|| opts.get("from"))
                .cloned(),
            quota,
            enable_compression: option_enabled(&opts, "compression", def.enable_compression),
            enable_atime: option_enabled(&opts, "atime", def.enable_atime),
            enable_exec: option_enabled(&opts, "exec", def.enable_exec),
            enable_setuid: option_enabled(&opts, "setuid", def.enable_setuid),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Zfs {
    root: PathBuf,
    mounts: Arc<CHashMap<String, HashSet<String>>>,
}

impl Zfs {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            mounts: Arc::new(CHashMap::new()),
        }
    }

    fn exists(&self, name: &str) -> Result<bool, Error> {
        self.get_mountpoint(name)
            .and(Ok(true))
            .or_else(|e| ignore_does_not_exist(e).map(|_| false))
    }

    fn do_create(&self, name: &str, opts: HashMap<String, String>) -> Result<(), Error> {
        let vopts = VolumeOptions::try_from(opts)?;
        Cmd::create(name, vopts).run(&self.root).and(Ok(()))
    }

    fn do_remove(&self, name: &str) -> Result<(), Error> {
        match self.mounts.get(name) {
            Some(ref by) if !by.is_empty() => Err(Error::VolInUseError(
                name.to_string(),
                by.iter().cloned().collect(),
            )),

            _ => Cmd::destroy(name).run(&self.root).and(Ok(())),
        }
    }

    fn do_mount(&self, name: &str, caller: &str) -> Result<PathBuf, Error> {
        Cmd::mount(name).run(&self.root).and_then(|_| {
            self.get_mountpoint(name).map(|mountpoint| {
                self.mounts.alter(name.to_string(), |old| {
                    let mut owners = old.unwrap_or_default();
                    owners.insert(caller.to_string());
                    Some(owners)
                });
                mountpoint
            })
        })
    }

    fn do_unmount(&self, name: &str, caller: &str) -> Result<(), Error> {
        if let Some(mut owners) = self.mounts.get_mut(name) {
            owners.remove(caller);
        }

        match self.mounts.get(name) {
            Some(ref by) if !by.is_empty() => Err(Error::VolInUseError(
                name.to_string(),
                by.iter().cloned().collect(),
            )),

            _ => Cmd::unmount(name).run(&self.root).and(Ok(())),
        }
    }

    fn get_mountpoint(&self, name: &str) -> Result<PathBuf, Error> {
        Cmd::get_mountpoint(name)
            .run(&self.root)
            .and_then(|stdout| {
                as_pathbuf(stdout).ok_or_else(|| Error::NoMountpointError(name.to_string()))
            })
    }

    fn inspect(&self, name: &str) -> Result<Dataset, Error> {
        let out = Cmd::inspect(name).run(&self.root)?;
        parse_dataset(&out)
            .map(|mut ds| {
                ds.name = Path::new(&ds.name)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                ds
            })
            .map_err(|e| e.into())
    }

    fn inspect_all(&self) -> Result<Vec<Dataset>, Error> {
        let out = Cmd::list().run(&self.root)?;
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(out.as_slice());

        let mut iter = rdr.deserialize();
        iter.next();

        let mut dss = Vec::new();
        for rs in iter {
            let ds: Result<Dataset, Error> = rs.map_err(|e| e.into());
            match ds {
                Ok(mut the_ds) => {
                    the_ds.name = Path::new(&the_ds.name)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned()
                        .to_string();
                    dss.push(the_ds)
                }
                Err(e) => return Err(e),
            }
        }
        Ok(dss)
    }
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Dataset {
    name: String,
    mountpoint: PathBuf,
    #[serde(with = "ts_seconds")]
    creation: DateTime<Utc>,
    used: u64,
    avail: u64,
}

fn parse_dataset(bs: &[u8]) -> Result<Dataset, csv::Error> {
    csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(bs)
        .deserialize()
        .next()
        .unwrap()
}

impl From<Dataset> for Volume {
    fn from(ds: Dataset) -> Self {
        Volume {
            name: ds.name,
            mountpoint: ds.mountpoint.to_str().map(String::from),
            created_at: Some(ds.creation),
            status: None,
        }
    }
}

impl VolumePlugin for Zfs {
    fn create(&self, rq: CreateRequest) -> Result<(), ErrorResponse> {
        info!("Volume.Create: {:?}", rq);
        if self.exists(&rq.name)? {
            Ok(())
        } else {
            self.do_create(&rq.name, rq.options.unwrap_or_default())
                .map_err(|e| e.into())
        }
    }

    fn remove(&self, rq: RemoveRequest) -> Result<(), ErrorResponse> {
        info!("Volume.Remove: {:?}", rq);
        self.do_remove(&rq.name).map_err(|e| e.into())
    }

    fn mount(&self, rq: MountRequest) -> Result<MountResponse, ErrorResponse> {
        info!("Volume.Mount: {:?}", rq);
        self.do_mount(&rq.name, &rq.id)
            .map_err(|e| e.into())
            .map(|mountpoint| MountResponse {
                mountpoint: mountpoint.to_str().map(String::from).unwrap(),
            })
    }

    fn path(&self, rq: PathRequest) -> Result<PathResponse, ErrorResponse> {
        info!("Volume.Path: {:?}", rq);
        self.get_mountpoint(&rq.name)
            .map_err(|e| e.into())
            .map(|mountpoint| PathResponse {
                mountpoint: mountpoint.to_str().map(String::from).unwrap(),
            })
    }

    fn unmount(&self, rq: UnmountRequest) -> Result<(), ErrorResponse> {
        info!("Volume.Unmount: {:?}", rq);
        self.do_unmount(&rq.name, &rq.id).map_err(|e| e.into())
    }

    fn get(&self, rq: GetRequest) -> Result<GetResponse, ErrorResponse> {
        info!("Volume.Get: {:?}", rq);
        self.inspect(&rq.name)
            .map_err(|e| e.into())
            .map(|ds| GetResponse { volume: ds.into() })
    }

    fn list(&self) -> Result<ListResponse, ErrorResponse> {
        info!("Volume.List");
        self.inspect_all()
            .map_err(|e| e.into())
            .map(|dss| ListResponse {
                volumes: dss.into_iter().map(|ds| ds.into()).collect(),
            })
    }

    fn capabilities(&self) -> CapabilitiesResponse {
        info!("Volume.Capabilities");
        CapabilitiesResponse {
            capabilities: Capabilities {
                scope: Scope::Local,
            },
        }
    }
}

// Helpers

// https://github.com/zfsonlinux/zfs/blob/ad0b23b14ab37a54764122fe8341e62f10245e15/cmd/zfs/zfs_main.c#L738
fn ignore_mount_error(e: Error) -> Result<(), Error> {
    ignore_stderr_msg(
        e,
        "filesystem successfully created, \
         but it may only be mounted by root",
    )
}

fn ignore_already_exists(e: Error) -> Result<(), Error> {
    ignore_stderr_msg(e, "dataset already exists")
}

fn ignore_does_not_exist(e: Error) -> Result<(), Error> {
    ignore_stderr_msg(e, "dataset does not exist")
}

fn ignore_stderr_msg(e: Error, msg: &str) -> Result<(), Error> {
    match e {
        Error::CmdError(_, ref stderr) => String::from_utf8_lossy(stderr)
            .rfind(msg)
            .map(|_| Ok(()))
            .unwrap_or(Err(e)),
        _ => Err(e),
    }
}

fn as_pathbuf(stdout: Vec<u8>) -> Option<PathBuf> {
    let s = String::from_utf8(stdout).expect("stdout not utf8");
    let l = s.lines().nth(0);
    match l {
        None | Some("none") | Some("") => None,
        Some(x) => Some(PathBuf::from(x)),
    }
}

fn sanitize_vol(vol: &str) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new("[^-_a-zA-Z0-9]").unwrap();
    }
    RE.replace_all(vol, "_").to_string()
}

trait CommandExt {
    fn run(&mut self) -> Result<Vec<u8>, Error>;
}

impl CommandExt for Command {
    fn run(&mut self) -> Result<Vec<u8>, Error> {
        match self.output() {
            Err(e) => Err(Error::CmdIoError(format!("{:?}", self), e)),
            Ok(out) => {
                if out.status.success() {
                    Ok(out.stdout)
                } else {
                    Err(Error::CmdError(format!("{:?}", self), out.stderr))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_dataset() {
        let data = "tank/zocker/tvol	/mnt/data/zocker/tvol	1566812157	98304	262045696";
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(data.as_bytes());

        let ds: Dataset = rdr.deserialize().next().unwrap().unwrap();
        assert_eq!(
            ds,
            Dataset {
                name: String::from("tank/zocker/tvol"),
                mountpoint: PathBuf::from("/mnt/data/zocker/tvol"),
                creation: Utc.timestamp(1566812157, 0),
                used: 98304,
                avail: 262045696,
            }
        )
    }

    #[test]
    fn test_sanitize_vol_noop() {
        let sane = vec!["foo", "foo_bar", "asd123-42", "-----"];
        for s in sane {
            assert_eq!(sanitize_vol(s), s)
        }
    }

    #[test]
    fn test_sanitize_vol_pathological() {
        assert_eq!(sanitize_vol("leboeuf:#42-fix-shit"), "leboeuf__42-fix-shit");
        assert_eq!(sanitize_vol("rename-libstd++11"), "rename-libstd__11");
        assert_eq!(sanitize_vol("ƒoo"), "_oo");
        assert_eq!(sanitize_vol("🗻∈🌏"), "___");
    }
}
