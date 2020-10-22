extern crate num_cpus;

use clap::ArgMatches;
use console::{style, Emoji};
use glob::glob;
use ini::Ini;
use std::env;
use std::fs::*;
use std::path::Path;
use std::os::unix::fs;
use std::path::*;
use std::process;
use std::process::Command;
use std::str;
use std::time::Instant;
use tar::Archive;
use tempdir::TempDir;

use indicatif::{HumanDuration, ProgressBar, ProgressStyle};

use crate::config;

// http://unicode.org/emoji/charts/full-emoji-list.html
static CHECKMARK: Emoji = Emoji("✅", "✅ ");
static FAIL: Emoji = Emoji("❌", "❌ ");
static WARNING: Emoji = Emoji("🚫", "🚫");

pub const BINS: [&str; 11] = [
    "bin/ct_run",
    "bin/dialyzer",
    "bin/epmd",
    "bin/erl",
    "bin/erlc",
    "bin/erl_call",
    "bin/escript",
    "bin/run_erl",
    "bin/run_test",
    "bin/to_erl",
    "bin/typer"
];

#[derive(Copy, Clone)]
enum BuildResult {
    Success,
    Fail,
}

struct CheckContext<'a> {
    src_dir: &'a Path,
    install_dir: &'a Path,
    build_status: BuildResult
}

enum CheckResult<'a> {
    Success,
    Warning(&'a str),
    Fail(&'a str),
}

enum BuildStep<'a> {
    Exec(&'a str, Vec<&'a str>),
    Check(Box<dyn Fn(&CheckContext) -> CheckResult<'a>>),
}

fn latest_tag(repo_dir: &str) -> String {
    let output = Command::new("git")
        .args(&["rev-list", "--tags", "--max-count=1"])
        .current_dir(repo_dir)
        .output()
        .unwrap_or_else(|e| {
            error!("git rev-list failed: {}", e);
            process::exit(1)
        });

    if !output.status.success() {
        error!(
            "finding latest tag of {} failed: {}",
            repo_dir,
            String::from_utf8_lossy(&output.stderr)
        );
        process::exit(1);
    }

    let rev = str::from_utf8(&output.stdout).unwrap();
    let output = Command::new("git")
        .args(&["describe", "--tags", &rev.trim()])
        .current_dir(repo_dir)
        .output()
        .unwrap_or_else(|e| {
            error!("git describe failed: {}", e);
            process::exit(1)
        });

    if !output.status.success() {
        error!(
            "describing latest tag of {} failed: {}",
            repo_dir,
            String::from_utf8_lossy(&output.stderr)
        );
        process::exit(1);
    }

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn update_bins(bin_path: &Path, links_dir: &Path) {
    let _ = std::fs::create_dir_all(links_dir);
    for &b in BINS.iter() {
        let f = Path::new(b).file_name().unwrap();
        let link = links_dir.join(f);
        debug!("linking {} to {}", link.display(), bin_path.display());
        let _ = std::fs::remove_file(&link);
        let _ = fs::symlink(bin_path, link);
    }
}

pub fn tags(sub_m: &ArgMatches, config: Ini) {
    let repo = sub_m.value_of("repo").unwrap_or("default");
    let git_repo = &config::lookup("repos", repo, &config).unwrap();
    let dir = &config::lookup_cache_dir(&config);
    let repo_dir = Path::new(dir).join("repos").join(repo);

    if !repo_dir.exists() {
        info!(
            "Cloning repo {} to {}",
            git_repo,
            repo_dir.to_str().unwrap()
        );
        clone_repo(git_repo, repo_dir.to_owned());
    }

    let output = Command::new("git")
        .args(&["tag"])
        .current_dir(repo_dir)
        .output()
        .unwrap_or_else(|e| {
            error!("git command failed: {}", e);
            process::exit(1)
        });

    if !output.status.success() {
        error!("tag failed: {}", String::from_utf8_lossy(&output.stderr));
        process::exit(1);
    }

    println!(
        "{}",
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    );
}

pub fn fetch(sub_m: &ArgMatches, config: Ini) {
    let repo = sub_m.value_of("repo").unwrap_or("default");
    let git_repo = &config::lookup("repos", repo, &config).unwrap_or_else(|| {
        error!("Repo {} not found in config", repo);
        process::exit(1)
    });
    let dir = &config::lookup_cache_dir(&config);
    let repo_dir = Path::new(dir).join("repos").join(repo);

    let started = Instant::now();
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}");

    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style);
    pb.enable_steady_tick(10);

    if !repo_dir.exists() {
        pb.set_message(&format!(
            "Cloning repo {} to {}",
            git_repo,
            repo_dir.to_str().unwrap()
        ));
        clone_repo(git_repo, repo_dir.to_owned());
        pb.println(format!(
            " {} Cloning repo {} to {:?}",
            CHECKMARK, git_repo, repo_dir
        ));
    }

    pb.set_message(&format!("Fetching tags from {}", git_repo));
    let output = Command::new("git")
        .args(&["fetch"])
        .current_dir(repo_dir)
        .output()
        .unwrap_or_else(|e| {
            error!("git fetch failed: {} {}", dir, e);
            process::exit(1)
        });

    if !output.status.success() {
        error!("fetch failed: {}", String::from_utf8_lossy(&output.stderr));
        process::exit(1);
    }

    pb.println(format!(" {} Fetching tags from {}", CHECKMARK, git_repo));
    pb.finish_and_clear();
    println!(
        "{} fetch in {}",
        style("Finished").green().bold(),
        HumanDuration(started.elapsed())
    );
}

fn clone_repo(git_repo: &str, repo_dir: std::path::PathBuf) {
    let _ = std::fs::create_dir_all(&repo_dir);
    let output = Command::new("git")
        .args(&["clone", git_repo, "."])
        .current_dir(&repo_dir)
        .output()
        .unwrap_or_else(|e| {
            error!("git clone failed: {:?} {}", repo_dir, e);
            process::exit(1)
        });

    if !output.status.success() {
        error!("clone failed: {}", String::from_utf8_lossy(&output.stderr));
        process::exit(1);
    }
}

pub fn run(bin_path: PathBuf, sub_m: &ArgMatches, config_file: &str, config: Ini) {
    let repo = sub_m.value_of("repo").unwrap_or("default");

    let repo_url = &config::lookup("repos", repo, &config).unwrap_or_else(|| {
        error!("Repo {} not found in config.\nTo add a repo: erlup repo add <name> <url>", repo);
        process::exit(1)
    });

    let dir = &config::lookup_cache_dir(&config);

    let key = "ERLUP_CONFIGURE_OPTIONS";
    let empty_string = &"".to_string();
    let user_configure_options = match env::var(key) {
        Ok(options) => options,
        _ => {
            config::lookup_with_default("erlup", "default_configure_options", empty_string, &config)
                .to_owned()
        }
    };
    let links_dir = Path::new(dir).join("bin");
    let repo_dir = Path::new(dir).join("repos").join(repo);
    let repo_dir_str = repo_dir.to_str().unwrap();

    let vsn = match sub_m.value_of("VSN").unwrap() {
        "latest" => latest_tag(repo_dir_str),
        vsn => vsn.to_string(),
    };

    let force: bool = sub_m.is_present("force");

    let id = sub_m.value_of("id").unwrap_or(&vsn);

    let install_dir = Path::new(dir).join("otps").join(id);
    let install_dir_str = install_dir.to_str().unwrap();

    if !install_dir.exists() || force {
        debug!("building {}:", id);
        debug!("    repo url: {}", repo_url);
        debug!("    repo dir: {}", repo_dir_str);
        debug!("    install: {}", install_dir_str);
        debug!("    version: {}", vsn);
        debug!("    options: {}", user_configure_options);
        debug!("    force: {}", force);
        build(
            repo_url,
            repo_dir_str,
            install_dir_str,
            &vsn,
            &user_configure_options,
        );
        update_bins(bin_path.as_path(), links_dir.as_path());

        // update config file with new built otp entry
        let dist = install_dir.join("dist");
        config::update(id, dist.to_str().unwrap(), config_file);
    } else {
        error!("Directory for {} already exists: {}", id, install_dir_str);
        error!("If this is incorrect remove that directory,");
        error!("provide a different id with --id <id> or provide --force.");
        process::exit(1);
    }
}

pub fn delete(_bin_path: PathBuf, sub_m: &ArgMatches, config_file: &str, config: Ini) {
    let dir = &config::lookup_cache_dir(&config);

    let id = sub_m.value_of("id").unwrap();

    let install_dir = Path::new(dir).join("otps").join(id);
    let install_dir_str = install_dir.to_str().unwrap();

    debug!("deleting {} at {}:", id, install_dir_str);

    // remove the entry from config
    config::delete(id, config_file);

    // delete the install dir from disk
    std::fs::remove_dir_all(install_dir_str).unwrap_or_else(|e| {
                                                    error!("unable to delete {} due to {}", install_dir_str, e);
                                                    process::exit(1);
                                                });
}

fn run_git(args: Vec<&str>) {
    let output = Command::new("git")
        .args(&args)
        .output()
        .unwrap_or_else(|e| {
            error!("git command failed: {}", e);
            process::exit(1)
        });

    if !output.status.success() {
        error!("clone failed: {}", String::from_utf8_lossy(&output.stderr));
        process::exit(1);
    }
}

fn clone(repo: &str, dest: &str) {
    run_git(vec!["clone", repo, dest]);
}

fn checkout(dir: &Path, repo_dir: &str, vsn: &str, pb: &ProgressBar) {
    let otp_tar = dir.join("otp.tar");
    debug!("otp_tar={}", otp_tar.to_str().unwrap());
    let output = Command::new("git")
        .args(&["archive", "-o", otp_tar.to_str().unwrap(), vsn])
        .current_dir(repo_dir)
        .output()
        .unwrap_or_else(|e| {
            error!("git archive failed: {}", e);
            process::exit(1)
        });

    if !output.status.success() {
        pb.println(format!(" {} {}",
                           FAIL,
                           format!("Checking out {}", vsn)));
        error!(
            "checkout of {} failed: {}",
            vsn,
            String::from_utf8_lossy(&output.stderr)
        );
        process::exit(1);
    }

    let mut ar = Archive::new(File::open(otp_tar).unwrap());
    ar.unpack(dir).unwrap();
}

fn setup_links(install_dir: &str) {
    for &b in BINS.iter() {
        let f = Path::new(b).file_name().unwrap();
        let bin = Path::new(install_dir).join("dist").join(b);
        let paths = glob(bin.to_str().unwrap()).unwrap();

        match paths.last() {
            Some(x) => {
                let link = Path::new(install_dir).join(f);
                let _ = fs::symlink(x.unwrap().to_str().unwrap(), link);
            }
            None => debug!("file to link not found: {}", f.to_str().unwrap()),
        }
    }
}

pub fn build(
    repo_url: &str,
    repo_dir: &str,
    install_dir: &str,
    vsn: &str,
    user_configure_options0: &str) {
    if !Path::new(repo_dir).is_dir() {
        clone(repo_url, repo_dir);
    }

    let started = Instant::now();
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}");

    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style);
    pb.enable_steady_tick(10);

    match TempDir::new("erlup") {
        Ok(dir) => {
            let num_cpus = &num_cpus::get().to_string();

            pb.set_message(&format!("Checking out {}", vsn));

            checkout(dir.path(), repo_dir, vsn, &pb);
            let _ = std::fs::create_dir_all(repo_dir);
            let _ = std::fs::create_dir_all(install_dir);

            pb.println(format!(" {} {} (done in {})",
                               CHECKMARK,
                               format!("Checking out {}", vsn),
                               HumanDuration(started.elapsed())
            ));
            debug!("temp dir: {:?}", dir.path());

            let dist_dir = Path::new(install_dir).join("dist");

            // split the configure options into a vector of String in a shell sensitive way
            // eg.
            //  from:
            //      user_configure_options0: --without-wx --without-observer --without-odbc --without-debugger --without-et --enable-builtin-zlib --without-javac CFLAGS="-g -O2 -march=native"
            //  to:
            //      user_configure_options: ["--without-wx", "--without-observer", "--without-odbc", "--without-debugger", "--without-et", "--enable-builtin-zlib", "--without-javac", "CFLAGS=-g -O2 -march=native"]
            let user_configure_options1: Vec<String> = shell_words::split(&user_configure_options0)
                                                                    .unwrap_or_else(|e| {
                                                                        error!("bad configure options {}\n\t{}", user_configure_options0, e);
                                                                        process::exit(1);
                                                                    });
            // build out a vector of &str
            let mut user_configure_options: Vec<&str> = user_configure_options1.iter()
                                                                               .map(|s| s as &str)
                                                                               .collect();
            // basic configure options must always include a prefix
            let mut configure_options = vec!("--prefix", dist_dir.to_str().unwrap());
            // append the user defined options
            configure_options.append(&mut user_configure_options);

            // declare the build pipeline steps
            let build_steps: [BuildStep; 8] = [
                BuildStep::Exec("./otp_build", vec!("autoconf")),
                BuildStep::Exec("./configure", configure_options),
                BuildStep::Check(Box::new(|context| {
                        if has_openssl(context.src_dir) {
                            CheckResult::Success
                        } else {
                            CheckResult::Warning("No usable OpenSSL found, please specify one with --with-ssl configure option, `crypto` application will not work in current build")
                        }
                    })),
                BuildStep::Exec("make", vec!("-j", num_cpus)),
                BuildStep::Exec("make", vec!("-j", num_cpus, "docs", "DOC_TARGETS=chunks")),
                // after `make` we'll already know if this build failed or not, this allows us
                // to make a better decision in wether to delete the installation dir should there
                // be one.
                BuildStep::Check(Box::new(|context| {
                        match context.build_status {
                            BuildResult::Fail => {
                                debug!("build has failed, aborting install to prevent overwriting a possibly working installation dir");
                                // this build has failed, we won't touch the previously existing install
                                // dir, for all we know it could hold a previously working installation
                                CheckResult::Fail("")
                            }
                            // if the build succeeded, then we check for an already existing
                            // install dir, if we find one we can delete it and proceed to the
                            // install phase
                            BuildResult::Success => {
                                // is install dir empty? courtesy of StackOverflow
                                let is_empty = context.install_dir.read_dir().map(|mut i| i.next().is_none()).unwrap_or(false);
                                if is_empty {
                                    // it's fine, it was probably us who created the dir just a moment ago,
                                    // that's why it's empty
                                    CheckResult::Success
                                } else {
                                    debug!("found a non empty installation dir after a successful build, removing it");
                                    // dir is not empty, maybe a working installation is already there,
                                    // delete the whole thing and proceed, we can go ahead with this 
                                    // because we know we have a working build in our hands
                                    let _ = std::fs::remove_dir_all(context.install_dir);
                                    CheckResult::Success
                                }
                            },
                        }
                    })),
                BuildStep::Exec("make", vec!("-j", num_cpus, "install")),
                BuildStep::Exec("make", vec!("-j", num_cpus, "install-docs")),
            ];
            // execute them sequentially
            let mut build_status = BuildResult::Success; 
            for step in build_steps.iter() {
                let step_started = Instant::now();

                match step {
                    BuildStep::Exec(command, args) => {
                        // it only takes one exec command to fail for the build status
                        // to be fail as well, a subsequent check build step can optionally decide
                        // to fail the pipeline
                        match exec(command, &args, dir.path(), step_started, &pb) {
                            BuildResult::Fail => {
                                build_status = BuildResult::Fail;
                            }
                            _ => ()
                        }
                    },
                    BuildStep::Check(fun) => {
                        let context = CheckContext{src_dir: dir.path(),
                                                   install_dir: Path::new(install_dir),
                                                   build_status: build_status};
                        match fun(&context) {
                            CheckResult::Success => {
                                debug!("success");
                            },
                            CheckResult::Warning(warning) => {
                                debug!("{}", warning);
                                pb.set_message(warning);
                                pb.println(format!(" {} {}", WARNING, warning));
                            },
                            CheckResult::Fail(_) => {
                                // abort
                                pb.finish_and_clear();
                                std::process::exit(1);
                            },
                        }
                    }
                }
            }
            // By closing the `TempDir` explicitly, we can check that it has
            // been deleted successfully. If we don't close it explicitly,
            // the directory will still be deleted when `tmp_dir` goes out
            // of scope, but we won't know whether deleting the directory
            // succeeded.
            drop(dir);

        }
        Err(e) => {
            error!("failed creating temp directory for build: {}", e);
        }
    }

    pb.set_message("Setting up symlinks");
    setup_links(install_dir);
    pb.println(format!(" {} {}", CHECKMARK, "Setting up symlinks"));

    pb.finish_and_clear();
    println!(
        "{} build in {}",
        style("Finished").green().bold(),
        HumanDuration(started.elapsed())
    );
}

fn exec(command: &str, args: &Vec<&str>,
        dir: &Path, started_ts: Instant,
        pb: &ProgressBar) -> BuildResult {
    debug!("Running {} {:?}", command, args);
    pb.set_message(&format!("{} {}", command, args.join(" ")));
    let output = Command::new(command)
        .env_clear()
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| {
            pb.println(format!(" {} {} {}",
                       FAIL,
                       command, args.join(" ")));
            error!("build failed: {}", e);
            process::exit(1)
        });

    debug!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    debug!("stderr: {}", String::from_utf8_lossy(&output.stderr));

    match output.status.success() {
        true => {
            pb.println(format!(" {} {} {} (done in {})",
                       CHECKMARK,
                       command, args.join(" "),
                       HumanDuration(started_ts.elapsed())));
            BuildResult::Success
        }
        false => {
            error!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            pb.println(format!(" {} {} {}",
                       FAIL,
                       command, args.join(" ")));
            BuildResult::Fail
        }
    }
}

fn has_openssl(src_dir: &Path) -> bool {
    // check that lib/crypto/SKIP doesn't exist,
    // if it does it means something went wrong with OpenSSL
    ! src_dir.join("./lib/crypto/SKIP").exists()
}

