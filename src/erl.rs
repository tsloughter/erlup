use std::path::*;
use std::env::Args;
use std::process::Command;
use std::os::unix::prelude::CommandExt;

use crate::config;

pub fn run(bin: &str, args: Args) {
    // no -c argument available in this case
    let erl_dir = config::erl_to_use();
    let cmd = Path::new(&erl_dir).join("bin").join(bin);

    debug!("running {}", cmd.to_str().unwrap());

    let _ = Command::new(cmd.to_str().unwrap()).args(args).exec();
}
