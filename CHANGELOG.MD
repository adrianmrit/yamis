# Changelog

## [Unreleased]
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