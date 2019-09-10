use std::borrow::BorrowMut;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fmt;
use std::fmt::Display;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use byte_unit::Byte;
use chrono::prelude::*;
use chrono::serde::ts_seconds;
use itertools::Itertools;
use serde::Deserialize;

use crate::api::*;

pub enum Cmd {
    Create,
    Destroy,
    Mount,
    Unmount,
    List,
    Snapshot,
    Clone,
    Promote,
}

impl Cmd {
    fn run<I, S>(self, args: I) -> Result<Vec<u8>, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new(if self.sudo() { "sudo" } else { "zfs" });
        let cmd0 = {
            let cmd1 = if self.sudo() {
                cmd.arg("zfs")
            } else {
                cmd.borrow_mut()
            };
            cmd1.arg(self.to_string()).args(args)
        };
        let out = cmd0.output()?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(Error::CmdError(format!("{:?}", cmd0), out.stderr))
        }
    }

    #[cfg(target_os = "linux")]
    fn sudo(&self) -> bool {
        match self {
            Cmd::Mount => true,
            Cmd::Unmount => true,
            _ => false,
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn sudo(&self) -> bool {
        false
    }
}

impl Display for Cmd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Cmd::Create => write!(f, "create"),
            Cmd::Destroy => write!(f, "destroy"),
            Cmd::Mount => write!(f, "mount"),
            Cmd::Unmount => write!(f, "unmount"),
            Cmd::List => write!(f, "list"),
            Cmd::Snapshot => write!(f, "snapshot"),
            Cmd::Clone => write!(f, "clone"),
            Cmd::Promote => write!(f, "promote"),
        }
    }
}

pub enum Error {
    IoError(io::Error),
    VolInUseError(String, Vec<String>),
    MountsLockError(String, String),
    CmdError(String, Vec<u8>),
    CmdOutputParseError(csv::Error),
    VolumeOptionsError(OptsError),
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
            Error::VolInUseError(vol, by) => vec![
                "Volume".to_string(),
                vol,
                "is in use by: ".to_string(),
                by.into_iter().join(","),
            ]
            .into_iter()
            .collect(),
            Error::MountsLockError(vol, e) => format!(
                "Could not acquire lock when trying to check mount status for volume {}: {}",
                vol, e
            ),
            Error::CmdError(cmd, stderr) => {
                format!("{}: {}", cmd, String::from_utf8_lossy(&stderr).into_owned())
            }
            Error::CmdOutputParseError(e) => e.to_string(),
            Error::VolumeOptionsError(e) => e.to_string(),
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

    fn as_args(&self, pre: Vec<String>) -> Vec<String> {
        let dasho = vec!["-o".to_string()];
        let props: Vec<String> = self
            .as_properties()
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .intersperse("-o".to_string())
            .collect();

        pre.into_iter().chain(dasho).chain(props).collect()
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
            .or_else(|e| match e {
                Error::CmdError(_, ref stderr) => String::from_utf8_lossy(stderr)
                    .rfind("dataset does not exist")
                    .map(|_| Ok(false))
                    .unwrap_or(Err(e)),
                _ => Err(e),
            })
    }

    fn do_create(&self, name: &str, opts: HashMap<String, String>) -> Result<(), Error> {
        let vopts = VolumeOptions::try_from(opts)?;

        match vopts.snapshot_of {
            Some(ref vol) => {
                let snap = self.root.join(vol).to_str().unwrap().to_owned() + "@" + name;
                Cmd::Snapshot.run(&[&snap]).map(|_| ())?;
                Cmd::Clone
                    .run(vopts.as_args(vec![snap.to_owned()]))
                    .map(|_| ())?;
                Cmd::Promote.run(&[snap]).map(|_| ())
            }
            None => Cmd::Create
                .run(vopts.as_args(vec![self.root.join(name).to_str().unwrap().to_string()]))
                .map(|_| ()),
        }
    }

    fn do_remove(&self, name: &str) -> Result<(), Error> {
        match self.mounts.try_lock() {
            Ok(ref mut mutex) => match mutex.get(name) {
                None => Cmd::Destroy.run(&[self.root.join(name)]).map(|_| ()),
                Some(by) => Err(Error::VolInUseError(
                    name.to_string(),
                    by.iter().cloned().collect(),
                )),
            },
            Err(e) => Err(Error::MountsLockError(name.to_string(), e.to_string())),
        }
    }

    fn do_mount(&self, name: &str, caller: &str) -> Result<PathBuf, Error> {
        match self.mounts.try_lock() {
            Ok(ref mut mutex) => {
                let mountpoint = if !mutex.contains_key(name) {
                    Cmd::Mount
                        .run(&[self.root.join(name)])
                        .map(|_| self.get_mountpoint(&name))?
                } else {
                    self.get_mountpoint(&name)
                };

                mutex
                    .entry(name.to_string())
                    .or_insert_with(HashSet::new)
                    .insert(caller.to_string());

                mountpoint
            }
            Err(e) => Err(Error::MountsLockError(name.to_string(), e.to_string())),
        }
    }

    fn do_unmount(&self, name: &str, caller: &str) -> Result<(), Error> {
        match self.mounts.try_lock() {
            Ok(ref mut mutex) => {
                mutex
                    .get_mut(name)
                    .and_then(|owners| Some(owners.remove(caller)));

                match mutex.get(name) {
                    Some(by) => Err(Error::VolInUseError(
                        name.to_string(),
                        by.iter().cloned().collect(),
                    )),
                    None => Cmd::Unmount.run(&[name]).map(|_| ()),
                }
            }
            Err(e) => Err(Error::MountsLockError(name.to_string(), e.to_string())),
        }
    }

    fn get_mountpoint(&self, name: &str) -> Result<PathBuf, Error> {
        let out = Cmd::List.run(&[
            "-H",
            "-o",
            "mountpoint",
            self.root.join(name).to_str().unwrap(),
        ])?;
        let stdout = String::from_utf8(out).expect("Invalid Utf8 on process stdout");
        Ok(PathBuf::from(stdout.lines().nth(0).expect("Empty result")))
    }

    fn inspect(&self, name: &str) -> Result<Dataset, Error> {
        let out = Cmd::List.run(&[
            "-H",
            "-p",
            "-o",
            "name,mountpoint,creation,used,avail",
            self.root.join(name).to_str().unwrap(),
        ])?;
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
        let out = Cmd::List.run(&[
            "-H",
            "-p",
            "-r",
            "-o",
            "name,mountpoint,creation,used,avail",
            self.root.to_str().unwrap(),
        ])?;

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
