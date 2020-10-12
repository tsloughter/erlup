use std::fs::*;
use std::path::*;
use std::process;
use ini::Ini;

fn home_config_file() -> String {
    let config_dir = match dirs::config_dir() {
        Some(d) => d,
        None => { error!("no home directory available"); process::exit(1) },
    };
    let cache_dir = match dirs::cache_dir() {
        Some(d) => d,
        None => { error!("no home directory available"); process::exit(1) },
    };

    let default_config = config_dir.join("erlup").join("config");
    let default_cache = cache_dir.join("erlup");

    let _ = create_dir_all(config_dir.join("erlup"));
    let _ = create_dir_all(cache_dir.join("erlup"));

    if !default_config.exists() {
        let mut conf = Ini::new();
        conf.with_section(Some("erlup".to_owned()))
            .set("dir", default_cache.to_str().unwrap());
        conf.with_section(Some("repos".to_owned()))
            .set("default", "https://github.com/erlang/otp");
        conf.write_to_file(&default_config).unwrap();
        info!("Created a default config at {:?}", default_config.to_owned());
    }

    default_config.to_str().unwrap().to_string()
}

pub fn home_config() -> (String, Ini) {
    let config_file = home_config_file();
    (config_file.to_owned(), read_config(config_file))
}

pub fn list() {
    let (_, config) = home_config();
    if let Some(erlup) = config.section(Some("erlangs")) {
        for s in erlup {
            let (k, v) = s;
            println!("{} -> {}", k, v);        
        }
    } else {
        println!("No Erlang releases installed.");
    }
}

pub fn erl_to_use() -> String {
    let (_, config) = home_config();

    let erl_to_use = match Ini::load_from_file("erlup.config") {
        Ok(cwd_config) => {
            debug!("Found ./erlup.config");
            match lookup("config", "erlang", &cwd_config) {
                Some(entry) => entry.clone(),
                None => {
                    error!("No Erlang entry found in erlup.config");
                    error!("Delete or update the config file");
                    process::exit(1)
                }
            }
        },
        Err(_) => {
            debug!("No ./erlup.config found, going to default");
            match lookup("erlup", "default", &config) {
                Some(entry) => entry.clone(),
                None => {
                    error!("No default Erlang set. Use `erlup default <id>`");
                    process::exit(1)
                }
            }
        }
    };

    debug!("Using Erlang with id {}", erl_to_use);
    match lookup("erlangs", &erl_to_use, &config) {
        Some(erl) => erl.clone(),
        None => {
            error!("No directory found for Erlang with id {} in config", erl_to_use);
            process::exit(1)
        }
    }
}

pub fn read_config(config_file: String) -> Ini {
    match Ini::load_from_file(config_file) {
        Ok(ini) => ini,
        Err(_) => {
            let (_, ini) = home_config();            
            ini
        }
    }
}

pub fn lookup_cache_dir(conf: &Ini) -> &String {
    let error_message = "The config file ~/.config/erlup/config is missing erlup.dir setting used for storing repos and built Erlang versions";
    lookup_or_exit("erlup", "dir", error_message, conf)
}

pub fn lookup<'a>(section: &str, key: &str, conf: &'a Ini) -> Option<&'a String> {
    debug!("reading section '{}' key '{}'", section, key);
    match conf.section(Some(section)) {
        Some(section) => section.get(key),
        None => None
    }
}

pub fn lookup_or_exit<'a>(section: &str, key: &str, msg: &str, conf: &'a Ini) -> &'a String {
    debug!("reading section '{}' key '{}'", section, key);
    let section = conf.section(Some(section)).unwrap();
    match section.get(key) {
        Some(v) => v,
        None => {
                 error!("{}", msg);
                 process::exit(1)
        }
    }
}

pub fn lookup_with_default<'a>(section: &str, key: &str, default: &'a str, conf: &'a Ini) -> &'a str {
    debug!("reading section '{}' key '{}'", section, key);
    let section = conf.section(Some(section)).unwrap();
    match section.get(key) {
        Some(v) => v,
        None => default
    }
}

pub fn update(id: &str, dir: &str, config_file: &str) {
    let mut config = Ini::load_from_file(config_file).unwrap();
    config.with_section(Some("erlangs".to_owned())).set(id, dir);
    config.write_to_file(config_file).unwrap();
}

pub fn delete(id: &str, config_file: &str) {
    let mut config = Ini::load_from_file(config_file).unwrap();
    config.with_section(Some("erlangs".to_owned())).delete(id);
    config.write_to_file(config_file).unwrap();
}

pub fn switch(id: &str) {
    let (_, config) = home_config();
    match lookup("erlangs", id, &config) {
        Some(_) => {
            let cwd_config = Path::new("erlup.config");
            { let _ = File::create(cwd_config); }
            let mut mut_config = Ini::load_from_file("erlup.config").unwrap();
            mut_config.with_section(Some("config".to_owned())).set("erlang", id);
            mut_config.write_to_file("erlup.config").unwrap();
            info!("Switched Erlang used in this directory to {}", id);
            info!("Wrote setting to file {}", "./erlup.config");
        },
        None => {
            error!("{} is not a configured Erlang install", id);
            process::exit(1)
        }
    }
}

pub fn add_repo(repo_id: &str, repo_url: &str, config_file: &str, mut config: Ini) {
    config.with_section(Some("repos".to_owned()))
        .set(repo_id, repo_url);
    config.write_to_file(config_file).unwrap();
}

pub fn get_repos(config: &Ini) -> Vec<(&String, &String)> {
    match config.section(Some("repos")) {
        Some(section) => {
            section.iter()
                .map(|(k, v)| {
                    (k, v)
                })
                .collect::<Vec<(&String, &String)>>()
        },
        None =>
            vec!()
    }
}

pub fn delete_repo(repo_id: &str, config_file: &str, mut config: Ini) {
    config.with_section(Some("repos".to_owned()))
        .delete(repo_id);
    config.write_to_file(config_file).unwrap();
}

pub fn set_default(id: &str) {
    let (_, mut config) = home_config();
    match lookup("erlangs", &id, &config) {
        Some(_) => {
            config.with_section(Some("erlup".to_owned()))
                .set("default", id);
            let config_file = home_config_file();
            config.write_to_file(&config_file).unwrap();
            info!("Default Erlang now {}", id);
        },
        None => {
            error!("{} is not a configured Erlang install, can't set it to default", id);
            process::exit(1)
        }
    }
}
