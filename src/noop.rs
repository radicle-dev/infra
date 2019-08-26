use crate::api::*;

#[derive(Copy, Clone)]
pub struct Noop;

impl VolumePlugin for Noop {
    fn create(&self, _: CreateRequest) -> Result<(), ErrorResponse> {
        Err("Create not implemented".into())
    }

    fn remove(&self, _: RemoveRequest) -> Result<(), ErrorResponse> {
        Err("Remove not implemented".into())
    }

    fn mount(self, _: MountRequest) -> Result<MountResponse, ErrorResponse> {
        Ok(MountResponse {
            mountpoint: "/dev/null".to_string(),
        })
    }

    fn path(&self, _: PathRequest) -> Result<PathResponse, ErrorResponse> {
        Ok(PathResponse {
            mountpoint: "/dev/null".to_string(),
        })
    }

    fn unmount(self, _: UnmountRequest) -> Result<(), ErrorResponse> {
        Err("unmount not implemented".into())
    }

    fn get(&self, rq: GetRequest) -> Result<GetResponse, ErrorResponse> {
        Ok(GetResponse {
            volume: Volume {
                name: rq.name,
                mountpoint: None,
                created_at: None,
                status: None,
            },
        })
    }

    fn list(self) -> Result<ListResponse, ErrorResponse> {
        Ok(ListResponse { volumes: vec![] })
    }

    fn capabilities(self) -> CapabilitiesResponse {
        CapabilitiesResponse {
            capabilities: Capabilities {
                scope: Scope::Local,
            },
        }
    }
}
