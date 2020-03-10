use std::env;
use std::path::Path;
use std::io::{Write};
use std::fs;
use std::collections::HashMap;
use std::fs::File as FsFile;
use dirs;
use config::{ConfigError, Config, File, Value};

pub struct Settings {
    config: Config,
    config_filename: String,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let s = Config::new();
        let mut settings = Settings {
            config: s,
            config_filename: String::new(),
        };

        // Create config dir if necessary
        match dirs::config_dir() {
            Some(mut dir) => {
                dir.push(env!("CARGO_PKG_NAME"));
                let dir = dir.into_os_string().into_string().unwrap();
                if !Path::new(&dir).exists() {
                    match fs::create_dir_all(dir.clone()) {
                        Err(why) => warn!("Could not create config dir: {}", why),
                        Ok(()) => ()
                    }
                }
            },
            None => { println!("Could not determine config dir"); }
        };

        let confdir: String = match dirs::config_dir() {
            Some(mut dir) => {
                dir.push(env!("CARGO_PKG_NAME"));
                dir.push("config");
                dir.into_os_string().into_string().unwrap()

            },
            None => { String::new() }
        };
        settings.config_filename = confdir.clone();
        println!("Looking for config file {}", confdir);

        // Set defaults
        settings.config.set_default("download_path", "Downloads")?;
        settings.config.set_default("homepage", "gopher://jan.bio:70/0/ncgopher/")?;
        settings.config.set_default("debug", false)?;

        if Path::new(confdir.as_str()).exists() {
            // Start off by merging in the "default" configuration file
            match settings.config.merge(File::with_name(confdir.as_str())) {
                Ok(_) => (),
                Err(e) => { warn!("Could not read config file: {}", e); },
            }
        }

        // Now that we're done, let's access our configuration
        println!("debug: {:?}", settings.config.get_bool("debug").unwrap());
        println!("homepage: {:?}", settings.config.get::<String>("homepage").unwrap());

        // You can deserialize (and thus freeze) the entire configuration as
        //s.try_into()
        Ok(settings)
    }

    pub fn write_settings_to_file(&mut self) -> std::io::Result<()> {
        let filename = self.config_filename.clone();
        info!("Saving settings to file: {}", filename);
        // Create a path to the desired file
        let path = Path::new(&filename);

        let mut file = match FsFile::create(&path) {
            Err(why) => return Err(why),
            Ok(file) => file,
        };

        match file.write(b"# Automatically generated by ncgopher.\n") {
            Err(why) => return Err(why),
            Ok(_) => (),
        }

        let config: HashMap<String, String> = match self.config.clone().try_into::<HashMap<String, String>>() {
            Ok(str) => str,
            Err(err) => {
                warn!("Could not write config: {}", err);
                HashMap::new()
            }
        };
        let toml = toml::to_string(&config).unwrap();
        file.write_all(toml.as_bytes())
    }

    pub fn set<T>(
        &mut self,
        key: &str,
        value: T
    ) -> Result<&mut Config, ConfigError> where
        T: Into<Value> {
        self.config.set::<T>(key, value)
    }

    /*
    pub fn get<'de, T: Deserialize<'de>>(&self, key: &'de str) -> Result<T, ConfigError> {
        self.config.get::<T>(key)
    }
    */

    pub fn get_str(&self, key: &str) -> Result<String, ConfigError> {
        println!("Asking for key {}", key);
        let res = self.config.get_str(key);
        println!("RES = {:?}", res);
        res
    }
}
