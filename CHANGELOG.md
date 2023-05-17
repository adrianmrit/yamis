# Changelog

## [2.0.0] - 

Because the usage is still low, I have decided to introduce some breaking changes
to improve the usability of the tool. Since the name of the binary has changed from
`yamis` to `mom`, it should be possible to keep using the old version.

### Added
- Add `--dry` option to print the commands that would be run without actually running them.
- Add `cmds` option to tasks to run multiple commands in a single task.
- Add `vars` option.
- Add `templates` option.

### Changed
- Drop the custom parser in favor of the [tera](https://tera.netlify.app/) template engine.
- Look for files in the following order: `yamis.private.yml`, `yamis.private.yaml`, `yamis.yml`, `yamis.yaml`, `yamis.root.yml`, `yamis.root.yaml`.
- Change the user config file to `~/yamis/yamis.global.yml` or `~/yamis/yamis.global.yaml`, in priority order.
- Can only run global task by using the `--global` or `-g` option.
- `script_runner` now takes a string instead of a list of strings.

### Removed
- Drop support for Toml files.
- Drop the guarantee of backward compatibility across mayor versions.
- Removed the `serial` task option.
- Removed the debug options.
- Removed the `script_runner_args` option.

## [1.2.0] - 2023-01-14

### Added
- Add option to print task name and config file path when running tasks.

### Changed
- A hash is generated and used as part of the name of the scripts saved in the temporal
 directory, so that they can be reused if the same script with same parameters is called
 again.

## [1.1.0] - 2022-10-15
### Added
- Upgrade to the latest version by running with the `--update` option.

### Changed
- Improve the update information process.
- Use rustls instead of openssl. Fixes some dependency issues on linux.

## [1.0.1] - 2022-10-11
### Changed
- Fix bug preventing `--file` option from working

## [1.0.0] - 2022-10-10
### Added
- Get help in the command like by calling yamis with the `--help` or `-h` option
- Get the version in the command line by calling yamis with `-V`
- Get the list of config files and tasks by calling yamis with `-t`
- Get basic info about a task by calling yamis with `-i <TASK>`
- Get list of task files by calling yamis with `-l` option
- Support for functions
- Support for index or slice expressions
- Support for global config files
- New release available notification
- Added a `help` field to tasks
- Preparing for future backward compatibility across mayor versions

### Changed
- Syntax changes
- Replaced `interpreter` argument with `script_runner` and `script_runner_args`
- Add `script_extension` alias for `script_ext`
- Use clap
- Use pest to parse scripts and arguments
- A program argument can contain either a task or a literal, not both at the same time
- Remove prefix and suffix feature in favor of functions
- Config files are lazily loaded
- Tasks inherit from os-specific bases if they exist
- Changed how all arguments and positional arguments are passed
- Improved and fixed error displaying
- Key-value arguments can be passed as `--key value` or `--key=value`

## [0.3.0] - 2022-08-28
### Added
- YAML 1.2 config files support.
- Add alias args+ for args_extend

## [0.2.0] - 2022-08-15
### Added
- Tasks can inherit from others.
- Can extend arguments from base tasks.
- Can specify interpreter and script extension in script tasks.
- Can specify env files at task or config level.
- Can load env variables into script and program arguments.

### Changed
- Fixed some error messages.
- Tasks that run multiple subtasks serially, stop when one of the subtasks fail.
- Display error message and exit code when a task fails.
- Better error message when setting invalid quote parameter.
- Better error messages overall.
- Fixed error where working directory could not be specified at the file level.

## [0.1.0] - 2022-07-30
### Added
- Initial release.
