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
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use byte_unit::Byte;
use chrono::prelude::*;
use chrono::serde::ts_seconds;
use itertools::Itertools;
use serde::Deserialize;
#[cfg(target_os = "linux")]
use users::get_effective_uid;

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
    fn run(&self, root: &PathBuf) -> Result<Vec<u8>, Error> {
        match self {
            Cmd::Create { vol, opts } => {
                let safe_vol = sanitize_vol(vol);
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
                                    .arg(root.join(safe_vol))
                            })
                            .or_else(|e| ignore_already_exists(e).and(Ok(Vec::new())))
                            .or_else(|e| ignore_mount_error(e).and(Ok(Vec::new())))?;
                        // finally, mark the snapshot for deletion
                        ZfsCmd::User.run(|zfs| zfs.arg("destroy").arg("-d").arg(&snap))
                    }
                    None => ZfsCmd::User
                        .run(|zfs| {
                            zfs.arg("create")
                                .args(opts.as_args())
                                .args(&["-o", "mountpoint=none"])
                                .arg(root.join(safe_vol))
                        })
                        .or_else(|e| ignore_mount_error(e).map(|_| Vec::new())),
                }
            }

            Cmd::Destroy { vol } => ZfsCmd::User.run(|zfs| {
                zfs.args(&["destroy", "-r"])
                    .arg(root.join(sanitize_vol(vol)))
            }),

            Cmd::Mount { vol } => {
                let safe_vol = sanitize_vol(vol);
                let root_mountpoint = ZfsCmd::User
                    .run(|zfs| {
                        zfs.args(&["get", "mountpoint", "-H", "-o", "value"])
                            .arg(root)
                    })
                    .and_then(|stdout| {
                        as_pathbuf(stdout).ok_or_else(|| {
                            Error::NoMountpointError(root.to_str().unwrap().to_string())
                        })
                    })?;
                let mountpoint = root_mountpoint.join(&safe_vol);

                // create mountpoint and adjust permissions
                fs::create_dir(&mountpoint)?;
                fs::set_permissions(&mountpoint, fs::Permissions::from_mode(0o750))?;

                ZfsCmd::Sudo.run(|zfs| {
                    zfs.arg("set")
                        .arg(format!("mountpoint={}", mountpoint.to_str().unwrap()))
                        .arg(root.join(safe_vol))
                })
            }

            Cmd::Unmount { vol } => {
                let safe_vol = sanitize_vol(vol);
                let mountpoint = Cmd::GetMountpoint {
                    vol: safe_vol.clone(),
                }
                .run(root)
                .and_then(|stdout| {
                    as_pathbuf(stdout).ok_or_else(|| Error::NoMountpointError(safe_vol.clone()))
                });

                ZfsCmd::Sudo.run(|zfs| {
                    zfs.args(&["set", "mountpoint=none"])
                        .arg(root.join(safe_vol))
                })?;

                if let Ok(dir) = mountpoint {
                    fs::remove_dir(dir)?;
                    Ok(Vec::new())
                } else {
                    Ok(Vec::new())
                }
            }

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
                    .arg(root.join(sanitize_vol(vol)))
            }),

            Cmd::Inspect { vol } => ZfsCmd::User.run(|zfs| {
                zfs.arg("list")
                    .args(&["-H", "-p", "-o", "name,mountpoint,creation,used,avail"])
                    .arg(root.join(sanitize_vol(vol)))
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
        let out = cmd.output()?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(Error::CmdError(format!("{:?}", cmd), out.stderr))
        }
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
}

pub enum Error {
    IoError(io::Error),
    VolInUseError(String, Vec<String>),
    MountsLockError(String, String),
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
    mounts: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

impl Zfs {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            mounts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn exists(&self, name: &str) -> Result<bool, Error> {
        self.get_mountpoint(name)
            .and(Ok(true))
            .or_else(|e| ignore_does_not_exist(e).map(|_| false))
    }

    fn do_create(&self, name: &str, opts: HashMap<String, String>) -> Result<(), Error> {
        let vopts = VolumeOptions::try_from(opts)?;
        Cmd::Create {
            vol: name.to_string(),
            opts: vopts,
        }
        .run(&self.root)
        .map(|_| ())
    }

    fn do_remove(&self, name: &str) -> Result<(), Error> {
        match self.mounts.try_lock() {
            Ok(ref mut mutex) => match mutex.get(name) {
                Some(by) if !by.is_empty() => Err(Error::VolInUseError(
                    name.to_string(),
                    by.iter().cloned().collect(),
                )),

                _ => Cmd::Destroy {
                    vol: name.to_string(),
                }
                .run(&self.root)
                .map(|_| ()),
            },

            Err(e) => Err(Error::MountsLockError(name.to_string(), e.to_string())),
        }
    }

    fn do_mount(&self, name: &str, caller: &str) -> Result<PathBuf, Error> {
        match self.mounts.try_lock() {
            Ok(ref mut the_mounts) => Cmd::Mount {
                vol: name.to_string(),
            }
            .run(&self.root)
            .and_then(|_| {
                self.get_mountpoint(name).map(|mountpoint| {
                    let owners = the_mounts
                        .entry(name.to_string())
                        .or_insert_with(HashSet::new);
                    owners.insert(caller.to_string());
                    mountpoint
                })
            }),

            Err(e) => Err(Error::MountsLockError(name.to_string(), e.to_string())),
        }
    }

    fn do_unmount(&self, name: &str, caller: &str) -> Result<(), Error> {
        match self.mounts.try_lock() {
            Ok(ref mut the_mounts) => {
                if let Some(owners) = the_mounts.get_mut(name) {
                    owners.remove(caller);
                }

                match the_mounts.get(name) {
                    Some(by) if !by.is_empty() => Err(Error::VolInUseError(
                        name.to_string(),
                        by.iter().cloned().collect(),
                    )),

                    _ => Cmd::Unmount {
                        vol: name.to_string(),
                    }
                    .run(&self.root)
                    .map(|_| ()),
                }
            }

            Err(e) => Err(Error::MountsLockError(name.to_string(), e.to_string())),
        }
    }

    fn get_mountpoint(&self, name: &str) -> Result<PathBuf, Error> {
        Cmd::GetMountpoint {
            vol: name.to_string(),
        }
        .run(&self.root)
        .and_then(|stdout| {
            as_pathbuf(stdout).ok_or_else(|| Error::NoMountpointError(name.to_string()))
        })
    }

    fn inspect(&self, name: &str) -> Result<Dataset, Error> {
        let out = Cmd::Inspect {
            vol: name.to_string(),
        }
        .run(&self.root)?;
        parse_dataset(&out)
            .map(|mut ds| {
                ds.name = Path::new(&ds.name)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
                    .to_string();
                ds
            })
            .map_err(|e| e.into())
    }

    fn inspect_all(&self) -> Result<Vec<Dataset>, Error> {
        let out = Cmd::List.run(&self.root)?;
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
    vol.replace("/", "_")
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
}
