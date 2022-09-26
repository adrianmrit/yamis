use colored::Colorize;
use update_informer::{registry, Check};

#[cfg(feature = "runtime")]
use yamis::cli::exec;

#[cfg(feature = "runtime")]
/// If there is a new version available, return the message to display to the user.
fn check_update_available() -> Option<String> {
    let pkg_name = env!("CARGO_PKG_NAME");
    let current_version = env!("CARGO_PKG_VERSION");
    let repo = env!("CARGO_PKG_REPOSITORY");

    #[cfg(not(test))]
    let informer = update_informer::new(registry::GitHub, pkg_name, current_version);

    #[cfg(test)]
    let informer =
        update_informer::fake(registry::GitHub, pkg_name, current_version, "999.999.999");

    if let Ok(Some(version)) = informer.check_version() {
        let current_version = format!("v{}", current_version).red();

        let msg = format!(
            "A new release of {pkg_name} is available: {current_version} -> {new_version}",
            pkg_name = pkg_name.italic().cyan(),
            current_version = current_version,
            new_version = version.to_string().green()
        );

        let releases_tag_url = if repo.ends_with('/') {
            format!(
                "{repo}releases/tag/{version}",
                repo = repo,
                version = version
            )
        } else {
            format!(
                "{repo}/releases/tag/{version}",
                repo = repo,
                version = version
            )
        };

        let releases_tag_url = releases_tag_url.yellow();
        Some(format!(
            "\n{msg}\n{url}\n",
            msg = msg,
            url = releases_tag_url
        ))
    } else {
        None
    }
}

#[cfg(feature = "runtime")]
fn main() {
    if let Some(update_msg) = check_update_available() {
        println!("{}", update_msg);
    }
    match exec() {
        Ok(_) => {}
        Err(e) => {
            let err_msg = e.to_string();
            let prefix = "[YAMIS]".bright_yellow();
            for line in err_msg.lines() {
                eprintln!("{} {}", prefix, line.red());
            }
            std::process::exit(1);
        }
    }
}

#[test]
fn test_update_available() {
    let update_available = check_update_available().unwrap();
    assert!(update_available.contains(&format!(
        "A new release of {pkg_name} is available: v{current_version} -> v999.999.999",
        pkg_name = env!("CARGO_PKG_NAME"),
        current_version = env!("CARGO_PKG_VERSION")
    )));
}
