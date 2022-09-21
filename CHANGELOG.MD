# Changelog

## []
## Added
- Get help in the command like by calling yamis with the --help or -h option
- Get the version in the command line by calling yamis with -V 
- Support for functions inside tasks
- Better error message parsing
- Can index or slice expressions
- Support for global config files

### Changed
- Use clap
- Use pest to parse scripts and arguments
- A program argument can contain either a task or a literal, not both at the same time
- Remove prefix and suffix feature in favor of functions
- Config files are lazily loaded
- Tasks inherit from os-specific bases if they exist
- Changed how all arguments and positional arguments are passed

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