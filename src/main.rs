extern crate clap;
extern crate pretty_env_logger;

use clap::{App, Arg};

use std::path::PathBuf;
use zockervols::server::run_server;
use zockervols::zfs::Zfs;

fn main() {
    pretty_env_logger::init();
    let opts = App::new("Zockervols")
        .author("Kim Altintop <kim@monadic.xyz>")
        .about("Manage Docker Volumes on ZFS")
        .arg(
            Arg::with_name("root")
                .long("zfs-root")
                .value_name("NAME")
                .help("Choose the ZFS root dataset. Docker volumes will be created as children of this.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("socket")
                .long("socket")
                .value_name("FILE")
                .help("Override the UNIX socket Zockervols is listening on")
                .takes_value(true),
        )
        .get_matches();

    run_server(
        opts.value_of("socket")
            .unwrap_or("/run/docker/plugins/zockervols.sock"),
        Zfs::new(PathBuf::from(
            opts.value_of("root").unwrap_or("tank/zocker"),
        )),
    )
}
