# Yamis
![build](https://github.com/adrianmrit/yamis/actions/workflows/test.yml/badge.svg)
[![License: GPL v3](https://img.shields.io/github/license/adrianmrit/yamis)

## Motivation
Besides wanted to learn Rust, I always struggled finding
a task runner that had what I needed (such as good argument
parsing) and team oriented.

## Install

The easiest way to install is with cargo.
```bash
cargo install --force yamis
```

TODO: Add binaries

## Quick Start
`project.yamis.toml` should be added at the root of a project. 
Here is an example of how it could look like:
```toml
[env]  # global env variables
DEBUG = "TRUE"

[tasks.say_hi]
script = "echo Hello {name}"  # name can be passed as --name=world, -name=world, or name="big world"  

[tasks.say_hi.env]  # can add env variables per task
DEBUG = "FALSE"

[tasks.folder_content]  # Default for linux and macos, can be individually specified like for windows.
script = "ls {path?}"  # path is an optional argument

[tasks.folder_content.windows]  # Task version for windows systems
script = "dir {path?}"

[tasks.project_content]
wd = ""  # Uses the dir where the config file appears as working dir
script = "ls"

[tasks.project_content.windows]
wd = ""
script = "dir"

[tasks.python]
program = "python"
args = ["{( -c )1?}"]  # Runs either the python interpreter, or an inline program if given
```

After having a config file, you can run a task by calling `yamis`, the name of the task, and any arguments, i.e.
`yamis say_hello name="big world"`. Passing the same argument multiple times will also add it multiple times, i.e.
`yamis say_hello name="person 1" --name="person 2"` is equivalent to `echo Hello person 1 person 2`


## Usage
### Task files discovery
The program will look at the directory where it was invoked and its parents until a `project.yamis.toml` is
discovered or the root folder is reached. Valid filenames are the following:
- `local.yamis.toml`: First one to look at for tasks. This one should hold private tasks and should not
  be committed to the repository.
- `yamis.toml`: Second one to look at for tasks. Should be used in sub-folders of a project for tasks specific
  to that folder and sub-folders.
- `project.yamis.toml`: Last one to look at for tasks. The file discovery stops when this one is found.

Note that you can have multiple `local.yamis.toml` and `yamis.toml` files in a project.


### Adding prefix and suffix
You can add a prefix and suffix surrounded by parenthesis after and before the argument name inside the tag, i.e.
`{(-o )file?(.txt)}`, if `file=sample` is passed, it will add `-o sample.txt` to the script.


### Script
The `script` value inside a task will be executed in the command line (defaults to cmd in Windows
and bash in Unix). Scripts can spawn multiple lines, and contain shell built-ins and programs. When
passing multiple arguments, they will be expanded by default, the common example would be the `"{*}"`
tag which expands to all the passed arguments.


##### Auto quoting
By default, all passed arguments are quoted (with double quotes).
This can be changed at the task or file level by specifying the
`quote` param, which can be either:
- `always`: Always quote arguments (default)
- `spaces`: Quote arguments if they contain spaces
- `never`: Never quote arguments

Although quoting prevents common errors like things breaking because a space,
it might fail in certain cases. This might be fixed in the future.


### Program
The `program` value inside a task will be executed as a separate process, with the arguments passed
on `args`. Note that each argument can contain at most one tag, that is, `{1}{2}` is not valid. When
passing multiple values, they are unpacked into the program arguments, i.e. `"{*}"` will result in
all arguments passed down to the program. Note also that if you add an argument like `-f={*}.txt` will
also be unpacked as expected, with the argument surrounded by the suffix and prefix.


#### Script vs Program:
Because escaping arguments properly can get really complex quickly, scripts are prone to fail if certain
arguments are passed. To prevent classic errors, arguments are quoted by default (see
[__Auto quoting__](https://github.com/adrianmrit/yamis#auto-quoting)), but this is not completely safe.
Also, each time a script runs, a temporal batch or cmd file is created. Not a big deal, but an extra step
nevertheless.

On the other hand, programs run in their own process with arguments passed directly to it, so there is no
need to escape them. The downside however, is that we cannot execute builtin shell commands such as `echo`,
and we need to define the arguments as a list.


### Specific Os Tasks
You can have a different OS version for each task. If a task for the current OS is not found, it will
fall back to the non os-specific task if it exists. I.e.
```toml
[task.ls] # Runs if not in windows 
script = "ls {*?}"

[task.ls.windows]  # Other options are linux and macos
script = "dir {*?}"
```

### Running tasks serially
One obvious option to run tasks one after the other is to create a script, i.e. with the following:
```
yamis say_hi
yamis say_bye
```

The other option is to use `script`, which should take a list of tasks to run in order, i.e.:
```toml
[tasks.greet]
script = ["say_hi", "say_bye"]
```
Note that any argument passed will be passed to both tasks equally.


### Other options:
 #### Environment variables
  Environment variables can be defined at the task level. These two forms are equivalent:
```toml
[tasks.echo]
env = {"DEBUG" = "TRUE"}

[tasks.echo.env]
DEBUG = "TRUE"
  ```
  They can also be passed globally
```toml
[env]
DEBUG = "TRUE"
```

 ##### Working directory
  By default, the working directory of the task is one where it was executed. This can be changed at the task level
  or root level, with `wd`. The path can be relative or absolute, with relative paths being resolved against the
  configuration file and not the directory where the task was executed, this means `""` can be used to make the
  working directory the same one as the directory for the configuration file.
