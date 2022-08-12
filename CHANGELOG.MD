# Changelog

## [Unreleased]
### Added
- Tasks can inherit from others
- Can extend arguments from base tasks
- Can specify interpreter and task extension in script tasks
- Can specify env files at task or config level

### Changed
- Fixed some error messages
- Tasks that run multiple subtasks serially stop when one of the subtasks fail
- Display error message and exit code when a task fails
- Better error message when setting invalid quote parameter
- Better error messages overall

## [0.1.0] - 2022-07-30
### Added
- Initial release