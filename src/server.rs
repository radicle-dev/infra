use listenfd::ListenFd;
use std::marker::Sync;
use std::path::Path;
use tokio::net::UnixListener;
use tokio::reactor::Handle;
use warp::Filter;

use crate::api::*;

pub fn run_server<P, H>(socket: P, handler: H)
where
    P: AsRef<Path> + ToString,
    H: VolumePlugin + Clone + Sync + Send + 'static,
{
    let handler = warp::any().map(move || handler.clone());

    let activate = warp::path(ACTIVATE)
        .and(handler.clone())
        .map(|h: H| warp::reply::json(&h.activate()));

    let create = warp::path(CREATE)
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(handler.clone())
        .and_then(|rq: CreateRequest, h: H| {
            h.create(rq)
                .map(|k| warp::reply::json(&k))
                .map_err(warp::reject::custom)
        })
        .recover(error_response);

    let get = warp::path(GET)
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(handler.clone())
        .and_then(|rq: GetRequest, h: H| {
            h.get(rq)
                .map(|k| warp::reply::json(&k))
                .map_err(warp::reject::custom)
        })
        .recover(error_response);

    let list = warp::path(LIST)
        .and(handler.clone())
        .and_then(|h: H| {
            h.list()
                .map(|k| warp::reply::json(&k))
                .map_err(warp::reject::custom)
        })
        .recover(error_response);

    let remove = warp::path(REMOVE)
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(handler.clone())
        .and_then(|rq: RemoveRequest, h: H| {
            h.remove(rq)
                .map(|k| warp::reply::json(&k))
                .map_err(warp::reject::custom)
        })
        .recover(error_response);

    let path = warp::path(PATH)
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(handler.clone())
        .and_then(|rq: PathRequest, h: H| {
            h.path(rq)
                .map(|k| warp::reply::json(&k))
                .map_err(warp::reject::custom)
        })
        .recover(error_response);

    let mount = warp::path(MOUNT)
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(handler.clone())
        .and_then(|rq: MountRequest, h: H| {
            h.mount(rq)
                .map(|k| warp::reply::json(&k))
                .map_err(warp::reject::custom)
        })
        .recover(error_response);

    let unmount = warp::path(UNMOUNT)
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(handler.clone())
        .and_then(|rq: UnmountRequest, h: H| {
            h.unmount(rq)
                .map(|k| warp::reply::json(&k))
                .map_err(warp::reject::custom)
        })
        .recover(error_response);

    let capabilities = warp::path(CAPABILITIES)
        .and(handler.clone())
        .map(|h: H| warp::reply::json(&h.capabilities()));

    let routes = warp::post2().and(
        activate
            .or(create)
            .or(get)
            .or(list)
            .or(remove)
            .or(path)
            .or(mount)
            .or(unmount)
            .or(capabilities),
    );

    let server = warp::serve(routes);

    let mut fds = ListenFd::from_env();
    if let Some(listener) = fds.take_unix_listener(0).unwrap() {
        server.run_incoming(
            UnixListener::from_std(listener, &Handle::default())
                .unwrap()
                .incoming(),
        )
    } else {
        // FIXME: set permissions to 700
        let listener = UnixListener::bind(&socket)
            .expect(format!("Can't bind to UNIX socket at {}", socket.to_string()).as_str());
        server.run_incoming(listener.incoming())
    }
}

fn error_response(rej: warp::Rejection) -> Result<impl warp::Reply, warp::Rejection> {
    if let Some(err) = rej.find_cause::<ErrorResponse>() {
        Ok(warp::reply::json(err))
    } else {
        Err(rej)
    }
}
