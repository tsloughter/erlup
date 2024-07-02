extern crate clap;

#[macro_use]
extern crate log;

use clap::{Args, Parser, Subcommand};
use console::style;
use log::{Level, LevelFilter, Record};
use std::env;
use std::io::Write;
use std::path::*;
use std::process;

mod build;
mod config;
mod erl;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(short, long)]
    config: Option<String>,

    #[command(subcommand)]
    subcommand: SubCommands,
}

#[derive(Subcommand)]
enum SubCommands {
    /// Update binary symlinks to erlup executable
    UpdateLinks,

    /// List installed Erlangs
    List,

    /// Fetch latest tags for repo
    Fetch(RepoArgs),

    /// List available tags to build for a repo
    Tags(RepoArgs),

    /// List available branches to build for a repo
    Branches(RepoArgs),

    /// Switch Erlang to use by id
    Switch(IdArgs),

    /// Set default Erlang to use by id
    Default(IdArgs),

    /// Deletes an Erlang by id
    Delete(IdArgs),

    /// Build and Erlang by branch of tag name
    Build(BuildArgs),

    /// Update repos to the config
    Repo(RepoSubCommands),
}

#[derive(Args)]
struct RepoArgs {
    /// Which Erlang repo to use for command
    #[arg(short, long)]
    repo: Option<String>,
}

#[derive(Args)]
struct IdArgs {
    /// Id of the Erlang
    id: String,
}

#[derive(Args)]
struct BuildArgs {
    /// Branch of tag of the Erlang repo
    git_ref: String,

    /// Id to give the Erlang build
    #[arg(short, long)]
    id: Option<String>,

    /// Which Erlang repo to use for command
    #[arg(short, long)]
    repo: Option<String>,

    /// Forces a build disregarding any previously existing ones
    #[arg(short, long)]
    force: Option<bool>,
}

#[derive(Args)]
struct RepoSubCommands {
    #[command(subcommand)]
    cmd: RepoCmds,
}

#[derive(Subcommand)]
enum RepoCmds {
    /// Add repo to the configuration
    Add(RepoAddArgs),

    /// Remove repo from the configuration
    Rm(RepoRmArgs),

    /// List available repos
    Ls,
}

#[derive(Args)]
struct RepoAddArgs {
    /// Name of the repo to add
    name: String,

    /// Url of the git repo for the repo
    repo: String,
}

#[derive(Args)]
struct RepoRmArgs {
    /// Name of the repo to remove
    name: String,
}

fn repo_or_default(maybe_repo: Option<String>) -> String {
    match maybe_repo {
        Some(repo) => repo,
        None => "default".to_string(),
    }
}

fn handle_command(bin_path: PathBuf) {
    let cli = Cli::parse();

    let (config_file, config) = match &cli.config {
        Some(file) => (file.to_owned(), config::read_config(file.to_owned())),
        None => config::home_config(),
    };
    debug!("config_file: {}", config_file);

    match &cli.subcommand {
        SubCommands::UpdateLinks => {
            debug!("running update links");
            let dir = &config::lookup_cache_dir(&config);
            let links_dir = Path::new(dir).join("bin");
            build::update_bins(bin_path.as_path(), links_dir.as_path());
        }
        SubCommands::List => {
            debug!("running list");
            config::list();
        }
        SubCommands::Fetch(RepoArgs { repo }) => {
            debug!("running fetch: repo={:?}", repo);
            build::fetch(repo.clone(), config);
        }
        SubCommands::Tags(RepoArgs { repo }) => {
            debug!("running list tags: repo={:?}", repo);
            build::tags(repo_or_default(repo.clone()), config);
        }
        SubCommands::Branches(RepoArgs { repo }) => {
            debug!("running list branches: repo={:?}", repo);
            build::branches(repo_or_default(repo.clone()), config);
        }
        SubCommands::Switch(IdArgs { id }) => {
            debug!("running switch: id={}", id);
            config::switch(id.as_str());
        }
        SubCommands::Default(IdArgs { id }) => {
            debug!("running default: id={}", id);
            config::set_default(id.as_str());
        }
        SubCommands::Delete(IdArgs { id }) => {
            debug!("running delete: id={}", id);
            build::delete(bin_path, id.clone(), &config_file, config);
        }
        SubCommands::Build(BuildArgs {
            git_ref,
            id,
            repo,
            force,
        }) => {
            debug!("running build: {} {:?} {:?} {:?}", git_ref, id, repo, force);

            let repo = repo_or_default(repo.clone());
            let repo_url = &config::lookup("repos", repo.clone(), &config).unwrap_or_else(|| {
                error!(
                    "Repo {} not found in config.\nTo add a repo: erlup repo add <name> <url>",
                    repo
                );
                process::exit(1)
            });

            let dir = &config::lookup_cache_dir(&config);
            let repo_dir = Path::new(dir).join("repos").join(repo.clone());

            let git_ref = match git_ref.as_str() {
                "latest" => build::latest_tag(repo_dir),
                _ => git_ref.clone(),
            };

            let id = id.clone().unwrap_or(git_ref.clone());
            let force = match force {
                Some(f) => *f,
                None => false,
            };
            build::run(
                bin_path,
                git_ref,
                id,
                repo,
                repo_url.clone(),
                force,
                &config_file,
                config,
            );
        }
        SubCommands::Repo(repo_sub_cmd) => match &repo_sub_cmd.cmd {
            RepoCmds::Add(RepoAddArgs { name, repo }) => {
                debug!("running repo add: name={} repo={}", name, repo);
                config::add_repo(name, repo, &config_file, config);
            }
            RepoCmds::Rm(RepoRmArgs { name }) => {
                debug!("running repo rm: name={}", name);
                config::delete_repo(name, &config_file, config);
            }
            RepoCmds::Ls => {
                debug!("running repo ls");
                let repos = config::get_repos(&config);
                for (id, url) in repos {
                    println!("{} -> {}", id, url);
                }
            }
        },
    }
}

fn setup_logging() {
    let format = |buf: &mut env_logger::fmt::Formatter, record: &Record| {
        if record.level() == Level::Error {
            writeln!(buf, "{}", style(format!("{}", record.args())).red())
        } else if record.level() == Level::Info {
            writeln!(buf, "{}", record.args())
        } else {
            writeln!(buf, "{}", style(format!("{}", record.args())).blue())
        }
    };

    let key = "DEBUG";
    let level = match env::var(key) {
        Ok(_) => LevelFilter::Debug,
        _ => LevelFilter::Info,
    };

    env_logger::builder()
        .format(format)
        .filter(None, level)
        .init();
}

fn main() {
    setup_logging();

    let mut args = env::args();
    let binname = args.next().unwrap();
    let f = Path::new(&binname).file_name().unwrap();

    if f.eq("erlup") {
        match env::current_exe() {
            Ok(bin_path) => {
                debug!("current bin path: {}", bin_path.display());
                handle_command(bin_path)
            }
            Err(e) => {
                println!("failed to get current bin path: {}", e);
                process::exit(1)
            }
        }
    } else {
        match build::BINS
            .iter()
            .find(|&&x| f.eq(Path::new(x).file_name().unwrap()))
        {
            Some(x) => {
                let bin = Path::new(x).file_name().unwrap();
                erl::run(bin.to_str().unwrap(), args);
            }
            None => {
                error!("No such command: {}", f.to_str().unwrap());
                process::exit(1)
            }
        }
    }
}
