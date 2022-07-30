# Yamis
![build](https://github.com/adrianmrit/yamis/actions/workflows/test.yml/badge.svg)
![License: GPL v3](https://img.shields.io/github/license/adrianmrit/yamis)

## Motivation
There are tools used to shorten the length of every-day commands us programmers need to run.
I have tried some of these tools, I specially liked how [doskey](https://docs.microsoft.com/en-us/windows-server/administration/windows-commands/doskey)
allowed to pass extra arguments, and the structure of [cargo-make,](https://github.com/sagiegurari/cargo-make/)
but they still didn't meet all my requirements. 

In short, this tool brings more powerful argument parsing, team oriented configuration, and hopefully more
features in the future.

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


### Running tasks serially
One obvious option to run tasks one after the other is to create a script, i.e. with the following:
```
yamis say_hi
yamis say_bye
```

The other option is to use `serial`, which should take a list of tasks to run in order, i.e.:
```toml
[tasks.greet]
serial = ["say_hi", "say_bye"]
```
Note that any argument passed will be passed to both tasks equally.


#### Script vs Program:
Because escaping arguments properly can get really complex quickly, scripts are prone to fail if certain
arguments are passed. To prevent classic errors, arguments are quoted by default (see
[__Auto quoting__](https://github.com/adrianmrit/yamis#auto-quoting)), but this is not completely safe.
Also, each time a script runs, a temporal batch or cmd file is created. Not a big deal, but an extra step
nevertheless.

On the other hand, programs run in their own process with arguments passed directly to it, so there is no
need to escape them. The downside however, is that we cannot execute builtin shell commands such as `echo`,
and we need to define the arguments as a list.

### Passing parameters to tasks
When calling a task, you can pass args to insert into the scripts or the argument of programs. These arguments,
or ___argument tags___ can have different forms:
- positional: passed by position, i.e. `{1}`, `{2}`, etc.
- named: case-sensitive and passed by name, i.e. `{out}`, `{file}`, etc. Note that any dash before the argument
is removed, i.e. if `--file=out.txt` is passed, `{file}` will accept it. Also note that the named argument passed
to the task will need to be in the form `<key>=<value>`, i.e. `-o out.txt` is not recognized as a named argument,
this is to prevent ambiguities as the parsing of arguments can change from application to application.
- all: defined by `{*}`, all arguments will be passed as they are.

#### Valid named argument tags
Named argument tasks must start with a letter, and be followed by any number of letters, digits, `-` or `_`.

#### Optional argument tags
Argument tags are mandatory by default, but they can be made optional by adding `?`, i.e. `{*?}`
does not raise an error if no arguments are given.

#### Adding prefix and suffix
Argument tags can also include a prefix or suffix, which will be only added if the argument was passed,
i.e. `{(--f=)file?(.txt)}` will result in `--file=out.txt` of a file parameter is passed. Note that
`{(--f=)file(.txt)}`, even though `file` is mandatory, it is useful if we want to unpack it (see next section). 
Also, you can include anything inside the prefix and suffix except newlines or brackets. Note that
parenthesis can be included in the prefix or suffix, only the surrounding ones will be excluded, i.e.
`{(()sample())}` will result in `(hello)` if `sample=hello` is passed.

#### Arguments unpacking
When the same named argument it passed multiple times, the program or script will include them multiple time.
For example, given the following tasks:

```toml
[tasks.say-hi]
script = "echo hello {person}"

[tasks.something]
program = "imaginary-program"
args = ["{(-o )f}"]
```

If we call `yamis hello person=John1 person=John2`, it will run `echo hello "John1" "John2"`.
Similarly, `yamis something --f=out1.txt out2.txt` will call `imaginary-program` with
`["-o out1.txt", "-o out2.txt""]`

### Specific Os Tasks
You can have a different OS version for each task. If a task for the current OS is not found, it will
fall back to the non os-specific task if it exists. I.e.
```toml
[task.ls] # Runs if not in windows 
script = "ls {*?}"

[task.ls.windows]  # Other options are linux and macos
script = "dir {*?}"
```


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


## Contributing

### Issues
Feel free to create issues to report bugs, ask questions or request new features.

### Contributing with code
Code contributions are welcome and can be in the form of, but not limited to, fixes, more tests, or
new features. You can fork the repository and make a pull request, just make sure the code is well tested.
Signed commits are preferred.