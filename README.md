# Yamis
![build](https://github.com/adrianmrit/yamis/actions/workflows/test.yml/badge.svg)
![License: GPL v3](https://img.shields.io/github/license/adrianmrit/yamis)

## Motivation
Although there are great similar tools out there, the ones I have tried lack some things
I was interested in, like good argument parsing, and team oriented configuration files.
For instance, some of these tools didn't let you pass extra arguments at all, while others were
limited to positional arguments or passing all arguments inline.


## Features
- Separate configuration files for teams and local development
- Powerful argument parsing
- Run scripts or programs with arguments
- Setting the interpreter for scripts
- Tasks can inherit from one or more tasks
- Pass extra named or positional arguments to the underlying command, and mix them with predefined arguments
- Pass multiple arguments with the same key
- Add extra environment variables or load an environment file
- Define task version for specific system
- Multiplatform (supports Windows, Linux and macOs)
- Simple syntax thanks to TOML files

## Planned features
- Support for YAML files
- Input missing arguments
- Distribute installers for easier installation


## Install

If you already have rustc and cargo installer, the easiest way to install is with:
```bash
cargo install --force yamis
```

Compiled binaries are also available for Windows, Linux and MacOs under
[releases](https://github.com/adrianmrit/yamis/releases/tag/v0.1.0).

## Quick Start
`project.yamis.toml` should be added at the root of a project. 
Here is an example TOML file to demostrate some features:
```toml
[env]  # global env variables
DEBUG = "FALSE"
DOCKER_CONTAINER = "sample_docker_container"

[tasks._debugable_task]
private = true   # cannot be invoked directly

[tasks._debugable_task.env]
DEBUG = "TRUE"  # Add env variables per task

[tasks.say_hi]
script = "echo Hello {name}"  # name can be passed as --name=world, -name=world, or name="big world"

[tasks.folder_content]  # Default for linux and macos, can be individually specified like for windows.
script = "ls {path?}"  # path is an optional argument

[tasks.folder_content.windows]  # Task version for windows systems
script = "dir {*}"  # Passes all arguments


[tasks.compose-run]
wd = ""  # Uses the dir where the config file appears as working dir
program = "docker-compose"
args = ["run", "{$DOCKER_CONTAINER}", "{*}"]   # This syntax for environment variables works both for windows and unix systems.

[tasks.compose-debug]
bases = ["compose-run", "_debugable_task"]  # Inherit from other tasks
args_extend = ["{$DEBUG?}"]  # Extends args from base task. Here DEBUG is an optional environment variable
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

#### ⚠️Warning :
Scripts are stored in a file in the temporal directory of the system and is the job of the OS to delete it,
however it is not guaranteed that that will be the case. So any argument passed will be stored in the
script file and could be persisted indefinitely.


##### Auto quoting
By default, all passed arguments are quoted (with double quotes).
This can be changed at the task or file level by specifying the
`quote` param, which can be either:
- `always`: Always quote arguments (default)
- `spaces`: Quote arguments if they contain spaces
- `never`: Never quote arguments

Although quoting prevents common errors like things breaking because a space,
it might fail in certain cases.


##### Replacing interpreter
By default, the interpreter in windows is CMD, and bash in unix systems. To use another interpreter you can
set the `interpreter` option in a task, which should be a list, where the first value is the interpreter
program, and the rest are values to pass before the actual script file generated.

You might also want to override the `script_ext` option, which is a string containing the extension for the
script file, and can be prepended with a dot or not. For some interpreter the extension does not matter, but
for others it does. In windows the extension defaults to `cmd`, and `sh` in unix.

Example:
```toml
# Python script that prints the date and time
[tasks.hello_world]
interpreter = ["python", "-c"]
script_ext = "py"  # or .py
script = """
from datetime import datetime

print(datetime.now())
"""
```

If using this feature frequently it would be useful to use inheritance to shorten the task. The above can become:
```toml
[tasks._python_script]
interpreter = ["python", "-c"]
script_ext = "py"  # or .py
private = true

[tasks.hello_world]
bases = ["_python_script"]
script = """
from datetime import datetime

print(datetime.now())
"""
```


### Program
The `program` value inside a task will be executed as a separate process, with the arguments passed
on `args`. Note that each argument can contain at most one tag, that is, `{1}{2}` is not valid. When
passing multiple values, they are unpacked into the program arguments, i.e. `"{*}"` will result in
all arguments passed down to the program. Argument like `-f={*}.txt` will be also unpacked as expected,
with the argument surrounded by the suffix and prefix.

When using inheritance, the arguments for the base can be extended by using `args_extend` instead of `args`.
This is useful for adding extra parameters without rewriting them.


###$ Running tasks serially
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

It is possible to execute the same task or end with infinite loops. 
This is not prevented since it can be bypassed by using a script.


#### Script vs Program:
Because escaping arguments properly can get really complex quickly, scripts are prone to fail if certain
arguments are passed. To prevent classic errors, arguments are quoted by default (see
[__Auto quoting__](https://github.com/adrianmrit/yamis#auto-quoting)), but this is not completely safe.
Also, each time a script runs, a temporal batch or cmd file is created. Not a big deal, but an extra step
nevertheless.

On the other hand, programs run in their own process with arguments passed directly to it, so there is no
need to escape them. These can also be extended more easily, like by extending the arguments.
The downside however, is that we cannot execute builtin shell commands such as `echo`,
and we need to define the arguments as a list.


### Common Options

#### Passing parameters to tasks
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
Also, an env file can be specified at the task or global level. The path will be relative to the config file unless it is
an absolute path.
```toml
env_file = ".env"

[tasks.some]
env_file = ".env_2"
```

If both `env_file` and `env` options are set at the same level, both will be loaded, if there are duplicate keys, `env` will
take precedence. Similarly, the global env variables and env file will be loaded at the task level even if these options
are also set there, with the env variables defined on the task taking precedence over the global ones.


#### Passing environment variables as arguments
Environment variables can be passed in `args`, `args_extend` or `scripts` similar to argument tags, i.e. `{$ENV_VAR}`
loads `ENV_VAR`. This works with environment variables defined in the config file or task, or in environment files
loaded with the `env_file` option. Although it is possible to pass environment variables to scripts using the native
syntax, it will not work for program arguments, and it is not multiplatform either.

Note that environment variables loaded this way are loaded when the script or
program arguments are parsed, i.e. the following will not work:

```toml
[tasks.sample]
# $SAMPLE is not set yet when the script is parsed
script = """
export SAMPLE=VALUE
echo {$SAMPLE}
"""
```

#### Os Specific Tasks
You can have a different OS version for each task. If a task for the current OS is not found, it will
fall back to the non os-specific task if it exists. I.e.
```toml
[task.ls] # Runs if not in windows 
script = "ls {*?}"

[task.ls.windows]  # Other options are linux and macos
script = "dir {*?}"
```

##### Working directory
By default, the working directory of the task is one where it was executed. This can be changed at the task level
or root level, with `wd`. The path can be relative or absolute, with relative paths being resolved against the
configuration file and not the directory where the task was executed, this means `""` can be used to make the
working directory the same one as the directory for the configuration file.


##### Task inheritance

A task can inherit from multiple tasks by adding a `bases` property, which should be a list names of tasks in
the same file. This works like class inheritance in common languages like Python, but not all values are 
inherited. 

The inherited values are:
- wd
- quote
- script
- interpreter
- script_ext
- program
- args
- serial
- env (the values are merged instead of overwriting)
- env_file (the values are merged instead of overwriting)

Values not inherited are:
- args_extend (added to `args` when parsing the child task,
 so the parent task would actually inherit `args`)
- private

The inheritance works from bottom to top, with childs being processed before the parents. Circular dependencies
are not allowed and will result in an error.

Examples:
```toml
[tasks.program]
program = "program"
args = ["{name}"]

[tasks.program_extend]
bases = ["program"]
args_extend = ["{phone}"]

[tasks.other]
env = {"KEY" = "VAL"}
args = ["{other_param}"]
private = true  # cannot be called directly, field not inherited

[tasks.program_extend_again]
bases = ["program_extend", "other"]
args_extend = ["{address}"]
```

In the example above, `program_extend_again` will be equivalent to
```toml
[tasks.program_extend_again]
program = "program"
env = {"KEY" = "VAL"}
args = ["{name}", "{phone}", "{address}"]
```

## Contributing

### Issues
Feel free to create issues to report bugs, ask questions or request new features.

### Contributing with code
Code contributions are welcome and can be in the form of, but not limited to, fixes, more tests, or
new features. You can fork the repository and make a pull request, just make sure the code is well tested.
Signed commits are preferred.