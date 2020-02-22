use std::fs::File;
use std::path::*;
use std::process;
use ini::Ini;

fn home_config_file() -> String {
    let base_dir = match dirs::home_dir() {
        Some(home) => home.join(".config"),
        None => { error!("no home directory available"); process::exit(1) },
    };
    let cache_dir = match dirs::home_dir() {
        Some(home) => home.join(".cache"),
        None => { error!("no home directory available"); process::exit(1) },
    };

    let default_config = base_dir.join("erls").join("config");
    let default_cache = cache_dir.join("erls");

    if !default_config.exists() {
        let mut conf = Ini::new();
        conf.with_section(Some("erls".to_owned()))
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
    (config_file.to_owned(), read_config(config_file.to_owned()))
}

pub fn list() {
    let (_, config) = home_config();
    if let Some(erls) = config.section(Some("erlangs")) {
        for s in erls {
            let (k, v) = s;
            println!("{} -> {}", k, v);        
        }
    } else {
        println!("No Erlang releases installed.");
    }
}

pub fn erl_to_use() -> String {
    let (_, config) = home_config();

    let erl_to_use = match Ini::load_from_file("erls.config") {
        Ok(cwd_config) => {
            debug!("Found ./erls.config");
            match lookup("config", "erlang", &cwd_config) {
                Some(entry) => entry.clone(),
                None => {
                    error!("No Erlang entry found in erls.config");
                    error!("Delete or update the config file");
                    process::exit(1)
                }
            }
        },
        Err(_) => {
            debug!("No ./erls.config found, going to default");
            match lookup("erls", "default", &config) {
                Some(entry) => entry.clone(),
                None => {
                    error!("No default Erlang set. Use `erls default <id>`");
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
    let error_message = "The config file ~/.config/erls/config is missing erls.dir setting used for storing repos and built Erlang versions";
    lookup_or_exit("erls", "dir", error_message, conf)
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

pub fn switch(id: &str) {
    let (_, config) = home_config();
    match lookup("erlangs", id, &config) {
        Some(_) => {
            let cwd_config = Path::new("erls.config");
            { let _ = File::create(cwd_config); }
            let mut mut_config = Ini::load_from_file("erls.config").unwrap();
            mut_config.with_section(Some("config".to_owned())).set("erlang", id);
            mut_config.write_to_file("erls.config").unwrap();
            info!("Switched Erlang used in this directory to {}", id);
            info!("Wrote setting to file {}", "./erls.config");
        }
        None => {
            error!("{} is not a configured Erlang install", id);
            process::exit(1)
        }
    }
}

pub fn set_default(id: &str) {
    let (_, mut config) = home_config();
    match lookup("erlangs", &id, &config) {
        Some(_) => {
            config.with_section(Some("erls".to_owned()))
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
