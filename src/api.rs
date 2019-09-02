use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::io;

use serde::{Deserialize, Serialize};
use chrono::prelude::*;

pub const ACTIVATE: &str = "Plugin.Activate";

pub const CREATE: &str = "VolumeDriver.Create";
pub const GET: &str = "VolumeDriver.Get";
pub const LIST: &str = "VolumeDriver.List";
pub const REMOVE: &str = "VolumeDriver.Remove";
pub const PATH: &str = "VolumeDriver.Path";
pub const MOUNT: &str = "VolumeDriver.Mount";
pub const UNMOUNT: &str = "VolumeDriver.Unmount";
pub const CAPABILITIES: &str = "VolumeDriver.Capabilities";

pub trait DockerPlugin {
    fn activate(&self) -> ActivateResponse;
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ActivateResponse {
    pub implements: Vec<PluginImplements>,
}

#[derive(Serialize, Deserialize)]
pub enum PluginImplements {
    #[serde(rename = "lowercase")]
    Authz,
    NetworkDriver,
    VolumeDriver,
}

impl<T: VolumePlugin> DockerPlugin for T {
    fn activate(&self) -> ActivateResponse {
        ActivateResponse {
            implements: vec![PluginImplements::VolumeDriver],
        }
    }
}

pub trait VolumePlugin: DockerPlugin {
    fn create(&self, rq: CreateRequest) -> Result<(), ErrorResponse>;
    fn remove(&self, rq: RemoveRequest) -> Result<(), ErrorResponse>;
    fn mount(&self, rq: MountRequest) -> Result<MountResponse, ErrorResponse>;
    fn path(&self, rq: PathRequest) -> Result<PathResponse, ErrorResponse>;
    fn unmount(&self, rq: UnmountRequest) -> Result<(), ErrorResponse>;
    fn get(&self, rq: GetRequest) -> Result<GetResponse, ErrorResponse>;
    fn list(&self) -> Result<ListResponse, ErrorResponse>;
    fn capabilities(&self) -> CapabilitiesResponse;
}

// CreateRequest is the pub structure that docker's requests are deserialized to.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct CreateRequest {
    pub name: String,
    #[serde(rename = "Opts")]
    pub options: Option<HashMap<String, String>>,
}

// RemoveRequest pub structure for a volume remove request
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct RemoveRequest {
    pub name: String,
}

// MountRequest pub structure for a volume mount request
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct MountRequest {
    pub name: String,
    #[serde(rename = "UPPERCASE")]
    pub id: String,
}

// MountResponse pub structure for a volume mount response
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct MountResponse {
    pub mountpoint: String,
}

// UnmountRequest pub structure for a volume unmount request
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct UnmountRequest {
    pub name: String,
    #[serde(rename = "UPPERCASE")]
    pub id: String,
}

// PathRequest pub structure for a volume path request
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct PathRequest {
    pub name: String,
}

// PathResponse pub structure for a volume path response
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct PathResponse {
    pub mountpoint: String,
}

// GetRequest pub structure for a volume get request
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct GetRequest {
    pub name: String,
}

// GetResponse pub structure for a volume get response
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct GetResponse {
    pub volume: Volume,
}

// ListResponse pub structure for a volume list response
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ListResponse {
    pub volumes: Vec<Volume>,
}

// CapabilitiesResponse pub structure for a volume capability response
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct CapabilitiesResponse {
    pub capabilities: Capabilities,
}

// Volume represents a volume object for use with `Get` and `List` requests
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Volume {
    pub name: String,
    pub mountpoint: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub status: Option<HashMap<String, String>>,
}

// Capability represents the list of capabilities a volume driver can return
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Capabilities {
    pub scope: Scope,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Local,
}

// ErrorResponse is a formatted error message that docker can understand
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ErrorResponse {
    pub err: String,
}

impl From<String> for ErrorResponse {
    fn from(err: String) -> Self {
        Self { err }
    }
}

impl From<&str> for ErrorResponse {
    fn from(err: &str) -> Self {
        Self {
            err: String::from(err),
        }
    }
}

impl From<io::Error> for ErrorResponse {
    fn from(err: io::Error) -> Self {
        Self {
            err: err.to_string(),
        }
    }
}

impl Display for ErrorResponse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.err)
    }
}

impl std::error::Error for ErrorResponse {
    fn description(&self) -> &str {
        "Error Response"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_volume() {
        let v = Volume {
            name: "foo".to_string(),
            mountpoint: Some("/mnt/data/foo".to_string()),
            created_at: None,
            status: None,
        };

        let ser = serde_json::to_string(&v).unwrap();
        println!("{}", ser);

        let de: Volume = serde_json::from_str(&ser).unwrap();
        println!("{:?}", de);

        assert_eq!(v, de);
    }
}
