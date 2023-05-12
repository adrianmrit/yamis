use crate::types::DynErrResult;
use colored::Colorize;
use self_update::cargo_crate_version;
use self_update::version::bump_is_greater;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use assert_fs::TempDir;
#[cfg(not(test))]
use directories::ProjectDirs;

const LATEST_RELEASE_URL: &str = "https://github.com/adrianmrit/yamis/releases/latest/";
const CHECK_INTERVAL: u64 = 60 * 60 * 24; // 1 day

/// Represents the cache file used to store the last update check time and latest version
/// available so that we don't check for updates too often.
struct UpdateCacheFile {
    /// Path to the cache file.
    path: PathBuf,
    /// The last time we checked for updates.
    latest_update: u64,
    /// The latest version available.
    latest_version: String,
}

impl UpdateCacheFile {
    /// Creates a new `UpdateCacheFile` instance.
    fn new() -> Self {
        let cache_path = Self::get_path();
        match Self::parse_cache_file(cache_path) {
            Some(cache_file) => cache_file,
            None => Self::default(),
        }
    }

    /// Creates a new `UpdateCacheFile` instance with default values.
    fn default() -> Self {
        let path = Self::get_path();
        let latest_update = 0;
        let latest_version = String::new();
        Self {
            path,
            latest_update,
            latest_version,
        }
    }

    /// Parses the file in the given path returning a new `UpdateCacheFile` instance.
    /// If the file is invalid it returns None
    fn parse_cache_file(path: PathBuf) -> Option<Self> {
        let content = std::fs::read_to_string(&path).ok()?;
        let mut lines = content.lines();
        let latest_update = lines.next()?.parse().ok()?;
        let latest_version = lines.next()?.to_string();
        let regex = regex::Regex::new(r"\d+\.\d+\.\d+").unwrap();
        if !regex.is_match(&latest_version) {
            return None;
        }
        Some(Self {
            path,
            latest_update,
            latest_version,
        })
    }

    /// Returns the path to the cache file.
    #[cfg(not(test))]
    fn get_path() -> PathBuf {
        let proj_dir = match ProjectDirs::from("", "", "yamis") {
            Some(proj_dir) => proj_dir,
            None => {
                // TODO: handle error
                eprintln!("Could not find project directory");
                std::process::exit(1);
            }
        };
        let cache_dir = proj_dir.cache_dir();
        cache_dir.join("last_update_check")
    }

    #[cfg(test)]
    fn get_path() -> PathBuf {
        let mut path = TempDir::new().unwrap().path().to_path_buf();
        path.push("last_update_check");
        path
    }

    /// Whether the cache file is outdated.
    fn outdated(&self) -> bool {
        let now = SystemTime::now();
        let now = now.duration_since(UNIX_EPOCH).unwrap().as_secs();
        now - self.latest_update > CHECK_INTERVAL
    }

    /// Updates and writes the cache file to disk.
    fn update(&mut self, latest_version: String) -> DynErrResult<()> {
        let now = SystemTime::now();
        let now = now.duration_since(UNIX_EPOCH).unwrap().as_secs();
        self.latest_update = now;
        self.latest_version = latest_version;
        let content = format!("{}\n{}", self.latest_update, self.latest_version);
        create_dir_all(self.path.parent().unwrap())?;
        std::fs::write(&self.path, content).map_err(|e| e.into())
    }
}

/// Checks for updates and returns the message to be printed to the user.
pub(crate) fn check_update_available() -> DynErrResult<Option<String>> {
    let mut cache_file = UpdateCacheFile::new();

    if cache_file.outdated() {
        // #[cfg(not(test))]
        {
            let releases = self_update::backends::github::ReleaseList::configure()
                .repo_owner("adrianmrit")
                .repo_name("yamis")
                .build()?
                .fetch()?;
            let latest_release = releases[0].clone();
            // The trim might be unnecessary but just in case
            cache_file.update(latest_release.version.trim_start_matches('v').to_string())?;
        }
        #[cfg(test)]
        {
            cache_file.update("999.999.999".to_string())?;
        }
    }

    let current_version = cargo_crate_version!();
    let msg = if bump_is_greater(current_version, &cache_file.latest_version)? {
        let current_version = format!("v{}", current_version).red();
        let msg = format!(
            "A new release of yamis is available: {current_version} -> {new_version}",
            current_version = current_version,
            new_version = &cache_file.latest_version.green()
        );
        let update_instructions =
            "To update, run `yamis --update` or manually install the new version".yellow();
        let msg = format!(
            "\n{msg}\n{url}\n\n{inst}\n",
            msg = msg,
            url = LATEST_RELEASE_URL.yellow(),
            inst = update_instructions
        );

        Some(msg)
    } else {
        None
    };

    Ok(msg)
}

/// Updates yamis to the latest version.
pub(crate) fn update() -> DynErrResult<()> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("adrianmrit")
        .repo_name("yamis")
        .bin_name("yamis")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use std::fs::File;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_update_cache_file_parse() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file_path = temp_dir.path().join("last_update_check");
        let mut file = File::create(&cache_file_path).unwrap();
        file.write_all(b"123456789\n0.0.1").unwrap();
        let cache_file = UpdateCacheFile::parse_cache_file(cache_file_path).unwrap();
        assert_eq!(cache_file.latest_update, 123456789);
        assert_eq!(cache_file.latest_version, "0.0.1");
    }

    #[test]
    fn test_update_cache_file_parse_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file_path = temp_dir.path().join("last_update_check");
        let mut file = File::create(&cache_file_path).unwrap();
        file.write_all(b"123456789\n1").unwrap();
        let cache_file = UpdateCacheFile::parse_cache_file(cache_file_path);
        assert!(cache_file.is_none());
    }

    #[test]
    fn test_update_cache_file_outdated() {
        let mut cache_file = UpdateCacheFile::default();
        cache_file.latest_update = 0;
        assert!(cache_file.outdated());
        let now = SystemTime::now();
        let now = now.duration_since(UNIX_EPOCH).unwrap().as_secs();
        cache_file.latest_update = now;
        assert!(!cache_file.outdated());
    }

    #[test]
    fn test_update_cache_file_update() {
        let temp_dir = TempDir::new().unwrap();
        let cache_file_path = temp_dir.path().join("last_update_check");
        let mut file = File::create(&cache_file_path).unwrap();
        file.write_all(b"123456789\n0.0.1").unwrap();
        let mut cache_file = UpdateCacheFile::parse_cache_file(cache_file_path.clone()).unwrap();
        cache_file.update("0.0.2".to_string()).unwrap();
        let cache_file = UpdateCacheFile::parse_cache_file(cache_file_path).unwrap();
        assert_eq!(cache_file.latest_version, "0.0.2");
        assert_ne!(cache_file.latest_update, 123456789);
    }

    #[test]
    fn test_update_cache_file_new_defaults() {
        let cache_file = UpdateCacheFile::new();
        assert_eq!(cache_file.latest_version, "");
        assert_eq!(cache_file.latest_update, 0);
    }

    #[test]
    fn test_check_update_available() {
        let msg = check_update_available().unwrap();
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("A new release of yamis is available"));
    }
}
