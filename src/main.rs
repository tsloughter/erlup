#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;

use std::env;
use std::path::*;
use std::process;
use clap::App;
use log::{LogRecord, LogLevel, LogLevelFilter};
use env_logger::LogBuilder;
use console::style;

mod config;
mod build;
mod erl;

fn handle_command(bin_path: PathBuf) {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let (config_file, config) = match matches.value_of("config") {
        Some(file) => (file.to_owned(), config::read_config(file.to_owned())),
        None => config::home_config()
    };
    debug!("config_file: {}", config_file);

    match matches.subcommand() {
        ("fetch", Some(sub_m)) => {
            build::fetch(sub_m, config);
        },
        ("tags", Some(sub_m)) => {
            build::tags(sub_m, config);
        },
        ("build", Some(sub_m)) => {
            build::run(bin_path, sub_m, &config_file, config);
        },
        ("update_links", _) => {
            let dir = &config::lookup_cache_dir(&config);
            let links_dir = Path::new(dir).join("bin");
            build::update_bins(bin_path.as_path(), links_dir.as_path());
        },
        ("list", _) => {
            config::list();
        },
        ("switch", Some(sub_m)) => {
            let id = sub_m.value_of("ID").unwrap();
            config::switch(id);
        },
        ("repo", Some(sub_m)) => {
            match sub_m.value_of("CMD") {
                Some("add") => {
                    let repo_id = sub_m.value_of("NAME").unwrap_or_else(|| {
                        error!("Bad command: `repo add` command must be given a repo name and url");
                        process::exit(1)
                    });
                    let repo_url = sub_m.value_of("REPO").unwrap_or_else(|| {
                        error!("Bad command: `repo add` command must be given a repo name and url");
                        process::exit(1)
                    });
                    config::add_repo(repo_id, repo_url, &config_file, config);
                },
                Some(cmd) => {
                    error!("Bad command: unknown repo subcommand `{}`", cmd);
                    error!("repo command must be given subcommand `add` or `rm`");
                    process::exit(1)
                },
                None => {
                    error!("Bad command: `repo` command must be given subcommand `add` or `rm`");
                    process::exit(1)
                }
            }

        },
        ("default", Some(sub_m)) => {
            let id = sub_m.value_of("ID").unwrap();
            config::set_default(id);
        },
        _ => { let _ = App::from_yaml(yaml).print_help(); },
    }
}

fn setup_logging() {
    let format = |record: &LogRecord| {
        if record.level() == LogLevel::Error {
            style(format!("{}", record.args())).red().to_string()
        }
        else if record.level() == LogLevel::Info {
            format!("{}", record.args())
        }
        else {
            style(format!("{}", record.args())).blue().to_string()
        }
    };
    let mut builder = LogBuilder::new();

    let key = "DEBUG";
    let level = match env::var(key) {
        Ok(_) => LogLevelFilter::Debug,
        _ => LogLevelFilter::Info,
    };

    builder.format(format).filter(None, level);
    builder.init().unwrap();
}

fn main() {
    setup_logging();

    let mut args = env::args();
    let binname = args.nth(0).unwrap();
    let f = Path::new(&binname).file_name().unwrap();

    if f.eq("erlup") {
        match env::current_exe() {
            Ok(bin_path) => {
                debug!("current bin path: {}", bin_path.display());
                handle_command(bin_path)
            },
            Err(e) => { println!("failed to get current bin path: {}", e); process::exit(1) },
        }
    } else {
        match build::BINS.iter().find(|&&x| f.eq(Path::new(x).file_name().unwrap())) {
            Some(x) => {
                let bin = Path::new(x).file_name().unwrap();
                erl::run(bin.to_str().unwrap(), args);
            },
            None => { error!("No such command: {}", f.to_str().unwrap()); process::exit(1) },
        }
    }
}
