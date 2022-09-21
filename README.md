# Yamis
![build](https://github.com/adrianmrit/yamis/actions/workflows/test.yml/badge.svg)
![License: GPL v3](https://img.shields.io/github/license/adrianmrit/yamis)

> Task runner for teams and individuals. Written in [Rust](https://www.rust-lang.org/).

## Index
* [Motivation](#motivation)
* [Features](#features)
* [Installation](#installation)
  * [Binary Release](#binary-release)
* [Quick Start](#quick-start)
* [Note about YAML and TOML files](#note-about-yaml-and-toml-files)
* [Usage](#usage)
  * [Task Files](#task-files)
  * [Script](#script)
    * [Auto Quoting](#auto-quoting)
    * [Replacing Interpreter](#replacing-interpreter)
  * [Program](#program)
  * [Running Tasks Serially](#running-tasks-serially)
  * [Script vs Program](#script-vs-program)
  * [Tags](#tags)
  * [Index and Slice](#index-and-slice)
  * [Parameter's Type](#parameters-type)
    * [Positional Parameters](#positional-parameters)
    * [Named Parameters](#named-parameters)
    * [All Parameters](#all-parameters)
    * [Environment Variables](#environment-variables)
    * [String Parameters](#string-parameters)
  * [Optional Expressions](#optional-expressions)
  * [Unpacking](#unpacking)
  * [Setting Environment Variables](#setting-environment-variables)
  * [Functions](#functions)
    * [map](#map-function)
    * [flat](#flat-function)
    * [join](#join-function)
    * [fmt](#fmt-function)
  * [OS Specific Tasks](#os-specific-tasks)
  * [Working Directory](#working-directory)
  * [Task Inheritance](#task-inheritance)
    * [Extending Program Arguments](#extending-program-arguments)
    * [Private Tasks](#private-tasks)
* [Contributing](#contributing)

<a name="motivation"></a>
## Motivation
This project started out of necessity and fun as I wanted to learn Rust, but have become.
my everyday tool for running tasks. It aims to be simple and powerful, both for individuals and teams,
specially those working on different platforms.

This tool was inspired on [cargo-make](https://github.com/sagiegurari/cargo-make) and
[doskey](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/doskey)

<a name="features"></a>
## Features
- Multiplatform (Windows, Linux and macOs)
- OS specific tasks
- YAML and TOML support
- Global, per-project, per-directory and private config files
- Powerful argument parsing
- Support for environment variables
- Modify arguments with functions
- Run scripts with different interpreters (like Python, Bash, etc)
- Task inheritance

<a name="installation"></a>
## Installation
If you have [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/cargo/) installed ([rust installation instructions](https://www.rust-lang.org/tools/install)). Then run:
```bash
cargo install --force yamis
```

Pro-tip: make sure `~/.cargo/bin` directory is in your `PATH` environment variable.

<a name="binary-release"></a>
### Binary Release:
Binaries are also available for Windows, Linux and macOS under
[releases](https://github.com/adrianmrit/yamis/releases/). To install, download the zip for your system, extract,
and copy the binary to the desired location. You will need to ensure the folder that contains the binary is available
in the `PATH`.

<a name="quick-start"></a>
## Quick Start
The first step is to add a YAML or TOML file in the project root, i.e. `project.yamis.yaml`.
 
Here is a sample YAML file to demonstrate some features:
```yaml
# project.yamis.yaml
env:  # global env variables
  DEBUG: "FALSE"
  DOCKER_CONTAINER: sample_docker_container

tasks:
  _debuggable_task:
    private: true   # cannot be invoked directly
    env:
      DEBUG: "TRUE"  # Add env variables per task

  say_hi:
    script: "echo Hello {name}"  # name can be passed as --name=world, -name=world, or name="big world"
  
  say_hi.windows:
    script: "echo Hello {name} from Windows"  # Task version for windows systems
  
  folder_content:  # Default for linux and macOS, can be individually specified like for windows.
    script: "ls {$1?}"  # Takes a single optional argument
    
    windows:  # Another way of specifying OS specific tasks
      script: "dir {$1?}"

  compose-run:
    wd: ""  # Working dir is the dir containing the config file
    program: "docker-compose"
    # `{$DOCKER_CONTAINER}` 
    args: [
      "run",
      "{$DOCKER_CONTAINER}",  # passes an environment variable into the program arguments
      "{ $@ }"  # passes all extra given arguments 
    ]

  compose-debug:
    bases: ["compose-run", "_debuggable_task"]  # Inherit from other tasks
    args+: ["{$DEBUG?}"]  # Extends args from base task. Here DEBUG is an optional environment variable
```

After having a config file, you can run a task by calling `yamis`, the name of the task, and any arguments, i.e.
`yamis say_hi name="world"`. Passing the same argument multiple times will also add it multiple times, i.e.
`yamis say_hi name="person 1" --name="person 2"` is equivalent to `echo Hello person 1 person 2`


<a name="note-about-yaml-and-toml-files"></a>
## Note about YAML and TOML files:
The parser used to process YAML files treats values as strings if a string is expected.

For example, the following examples are equivalent
```yaml
env:
  DEBUG: Yes  # Normally becomes true
  AGENT: 007  # Normally becomes 7
```
```yaml
env:
  DEBUG: "Yes"
  AGENT: "007"
```

However, in the case of TOML files, the parser returns the appropriate type, and therefore it will result
in errors.

I.e. the following is not valid
```toml
[env]
    AGENT = 007
```

We do not implicitly perform this conversion because we would need to modify the TOML parser.
If we performed the conversion after parsing the file we would get `AGENT=7` which might be undesired.

<a name="usage"></a>
## Usage

<a name="task-files"></a>
### Task files
The task files must be either a TOML or YAML file with the appropriate extension, i.e. `project.yamis.toml`, or
`project.yamis.yml`. Note that across this document examples are given in either version, but the conversion between
them is straightforward.

When invoking a task, starting in the working directory and continuing to the root directory, the program will
look configuration files in a certain order until either a task is found, a `project.yamis` (either TOML or YAML)
task file is found, or there are no more parent folders (reached root directory). The name of these files is
case-sensitive in case-sensitive systems, i.e. `PROJECT.yamis.toml` will not work in linux.

The configuration files (in order of precedence, with extension omitted) are named as following:
- `local.yamis`: Should hold private tasks and should not be committed to the repository.
- `yamis`: Should be used in sub-folders of a project for tasks specific to that folder and sub-folders.
- `project.yamis`: Should hold tasks for the entire project.

If none of those files is found, it will look at `~/.yamis/user.yamis.toml` or `~/.yamis/user.yamis.yaml` or
`~/.yamis/user.yamis.yml` for user-wide tasks. This is useful for everyday tasks not related to a specific project.


<a name="script"></a>
### Script
The `script` value inside a task will be executed in the command line (defaults to cmd in Windows
and bash in Unix). Scripts can spawn multiple lines, and contain shell built-ins and programs. When
passing multiple arguments, they will be expanded by default, the common example would be the `"{ $@ }"`
tag which expands to all the passed arguments.

**⚠️Warning:**
Scripts are stored in a file in the temporal directory of the system and is the job of the OS to delete it,
however it is not guaranteed that that will be the case. So any argument passed will be stored in the
script file and could be persisted indefinitely.

<a name="auto-quoting"></a>
#### Auto quoting
By default, all passed arguments are quoted (with double quotes).
This can be changed at the task or file level by specifying the
`quote` param, which can be either:
- `always`: Always quote arguments (default)
- `spaces`: Quote arguments if they contain spaces
- `never`: Never quote arguments

Although quoting prevents common errors like things breaking because an argument with a space was passed,
it might fail in certain edge cases.

<a name="replacing-interpreter"></a>
#### Replacing interpreter
By default, the interpreter in windows is CMD, and bash in unix systems. To use another interpreter you can
set the `interpreter` option in a task, which should be a list, where the first value is the interpreter
program, and the rest are extra parameters to pass before the script file parameter.

You might also want to override the `script_ext` option, which is a string containing the extension for the
script file, and can be prepended with a dot or not. For some interpreter the extension does not matter, but
for others it does. In windows the extension defaults to `cmd`, and `sh` in unix.

Example:
```toml
# Python script that prints the date and time
[tasks.hello_world]
interpreter = ["python"]
script_ext = "py"  # or .py
script = """
from datetime import datetime

print(datetime.now())
"""
```

If using this feature frequently it would be useful to use inheritance to shorten the task. The above can become:
```toml
[tasks._py_script]
interpreter = ["python"]
script_ext = "py"  # or .py
private = true

[tasks.hello_world]
bases = ["_py_script"]
script = """
from datetime import datetime

print(datetime.now())
"""
```

<a name="program"></a>
### Program
The `program` value inside a task will be executed as a separate process, with the arguments passed
on `args`. Note that each argument can contain at most one tag, that is, `{$1}{$2}` is not valid. When
passing multiple values, they are unpacked into the program arguments, i.e. `"{$@}"` will result in
all arguments passed down to the program.

When using inheritance, the arguments for the base can be extended by using `args_extend` instead of `args`.
This is useful for adding extra parameters without rewriting them.


<a name="running-tasks-serially"></a>
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

It is possible to execute the same task or end with infinite loops. 
This is not prevented since it can be bypassed by using a script.


<a name="script-vs-program"></a>
### Script vs Program:
Because escaping arguments properly can get really complex quickly, scripts are prone to fail if certain
arguments are passed. To prevent classic errors, arguments are quoted by default (see
[__Auto quoting__](https://github.com/adrianmrit/yamis#auto-quoting)), but this is not completely safe.
Also, each time a script runs, a temporal batch or cmd file is created. Not a big deal, but an extra step
nevertheless.

On the other hand, programs run in their own process with arguments passed directly to it, so there is no
need to escape them. These can also be extended more easily, like by extending the arguments.
The downside however, is that we cannot execute builtin shell commands such as `echo`,
and we need to define the arguments as a list.


<a name="tags"></a>
### Tags
Tags are used to insert dynamic values into the scripts and arguments of program we want to call. Tags can be
used to insert positional and named arguments, environment variables (with a cross-platform syntax) and invoke
functions.

The expressions inside tasks can return values either as a string, or as a list of strings. If no values are passed,
the value will be an empty list, or an empty string in the case of positional arguments. This is specially relevant
when slicing and invoking functions.


<a name="index-and-slice"></a>
### Index and Slice
Arguments (more on arguments below) can be sliced for more flexibility. The slices are 0 indexed, here are some examples:

```text
{ $@[0] }                         # same as { $1 }

{ $@[0..2] }                      # first two arguments

{ map(f"hello {}", name)[0..2] }  # same as { map(f"hello {}", name[0..2]) }

{ fmt(f"hello {}", $1)[0] }       # returns `h`

{ $1[0] }                         # returns first char of first argument

{ $@[0][0] }                      # also returns first char of first argument
```


<a name="parameters-type"></a>
### Parameter's Type

<a name="positional-parameters"></a>
#### Positional Parameters
1-indexed, start with `$` and followed by a number, i.e. `{$1}`, `{$2}`. Represent a single string, so slices of them
will return a substring.

<a name="named-parameters"></a>
#### Named Parameters
Case-sensitive and passed by name, i.e. `{out}`, `{file}`, etc. Note that any dash before the argument
is removed, i.e. if `--file=out.txt` is passed, `{file}` will accept it. Also note that the named argument passed
to the task will need to be in the form `<key>=<value>`, i.e. `-o out.txt` is not recognized as a named argument,
this is to prevent ambiguities as the parsing of arguments can change from application to application.

Named argument must start with an ascii alpha character or underscore, and should be followed by any number
of letters, digits, `-` or `_`.

These are represented by arrays of strings, so an index slice will return a string, while a range slice will return
a subarray. I.e. `{ file[0][0] }` returns the first character of the first passed `file` argument, while `file[0]`
will return the first file argument.

<a name="all-parameters"></a>
#### All Parameters
With `{ $@ }`, all arguments will be passed as they are. They can be accessed by index and sliced, i.e. `{ $@[0] }` and
`{ $@[0..2] }` are valid. Can also be optional, i.e. `{ $@? }`.


<a name="environment-variables"></a>
#### Environment Variables
Prefixed with `$`, i.e. `{ $HOME }`, `{ $PATH }`, etc. These are represented as strings, so slices will return
a substring. Do not confuse with positional arguments, which are numeric, or with the all parameters syntax `{ $@ }`.

Note that with this syntax environment variables are loaded when the script or program arguments are parsed, unlike
the native syntax that will not work in arguments of programs, and in the case of scripts, will be loaded by the shell.
This is intentional, as you can keep them separate, or use env variables with a different
interpreter like python.


<a name="string-parameters"></a>
#### String Parameters
Strings are another type of valid expressions, but they are more relevant in the
function's context. Strings are defined by single or double quotes, cannot contain unescaped new lines.
I.e. `{ "\"hello\" \n 'world'" }` is a valid string. Strings can also be sliced, but this is side effect of trying
to keep the parser simple rather than a useful feature.

<a name="optional-expressions"></a>
### Optional Expressions
By default, expressions must return a non-empty string or non-empty array of strings, otherwise an error will be raised.
Expressions can be made optional by adding `?`, i.e. `{ $1? }`, `{ map("hello {}", person?)? }`, `{ $@? }`, `{ output? }`.

<a name="unpacking"></a>
### Unpacking
Expressions that return an array will be unpacked. For example, given the following tasks:

```toml
[tasks.say-hi]
script = "echo hello {person}"

[tasks.something]
program = "imaginary-program"
args = ["{ map('-o {}', f) }"]  # map returns an array of strings
```

If we call `yamis hello person=John1 person=John2`, it will run `echo hello "John1" "John2"`.
Similarly, `yamis something --f=out1.txt out2.txt` will call `imaginary-program` with
`["-o out1.txt", "-o out2.txt""]`. You might have noticed we call a `map`, more on functions later.


<a name="setting-environment-variables"></a>
### Setting Environment Variables
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


<a name="functions"></a>
### Functions
Predefined functions can be used to transform arguments in different ways. They can take values and can be
nested.

Functions can take string or array values, and also return either a single string or an array.


<a name="map-function"></a>
#### map Function
**Signature:** `map(fmt_string: str, values: str[]) -> str[]`

Maps each value to `fmt(fmt_string, val)`, where `fmt` replaces `{}` with value. Note that brackets
can be escaped by duplicating them, i.e. `{{` will be replaced with `{`

**Parameters:**
- `fmt_string`: String to format, i.e. `"-o {}.txt"`
- `values`: Values to map

Example:
```yaml
sample:
  quote: never
  script: |
    echo {map("'{}'", $@)}


sample2:
  program: merge_txt_files
  args: ["{map('{}.txt', $@)}"]
```

`yamis sample person1 person2` will result in `echo hi 'person1' 'person2'`

`yamis sample2 file1 file2` will result in calling `merge_txt_files` with arguments `["file1.txt", "file2.txt"]`


<a name="flat-function"></a>
#### flat Function
**Signature:** `flat(fmt_string: str, values: str[]) -> str`

`flat` is similar to map, but in scripts extra spaces won't be added, and in arguments it will not be unpacked. This is
because calling `flat` is like calling `map` and joining the resulting array values into a single string.

**Parameters:**
- `fmt_string`: String to format, i.e. `"-o {}.txt"`
- `values`: Values to map

Example:
```yaml
sample:
  quote: never
  script: |
    echo hi{flat(" '{}'", $@)}


sample2:
  program: some_program
  args: ["{flat('{},', $@)}"]
```

`yamis sample person1 person2` will result in `echo hi 'person1' 'person2' `

`yamis sample2 arg1 arg2` will result in calling `some_program` with arguments `["arg1,arg2,"]`


<a name="join-function"></a>
#### join Function
**Signature**: `join(join_str: str, values: str[]) -> str`

The first parameter of `join` is a string that will be inserted between all values given in the second parameter
returning a single string.

**Parameters:**
- `join_str`: String to insert between the values
- `values`: Values to join

Example:
```yaml
sample:
  quote: never
  script: |
    echo hello {flat(" and ", $@)}
```

`yamis sample person1 person2` will result in `echo hi person1 and person2'`


<a name="fmt-function"></a>
#### fmt Function
**Signature**: `fmt(fmt_string: str, ...args: str[]) -> str`

The first parameter of `fmt` is a format string, and the rest of the values are parameters to format the string with.
Note that those extra parameters must be i individual values, not arrays, i.e. cannot use `$@`.

**Parameters:**
- `fmt_string`: String to format, i.e. `"-o {}.txt"`
- `args`: Arguments that will replace the `{}` occurrence of the same index

Example:
```yaml
sample:
  quote: never
  script: |
    echo {fmt("Hi {} and {}", $1, $2)}
```

`yamis sample person1 person2` will result in `echo Hi person1 and person2`


<a name="os-specific-tasks"></a>
### OS Specific Tasks
You can have a different OS version for each task. If a task for the current OS is not found, it will
fall back to the non os-specific task if it exists. I.e.
```yaml
tasks:
  ls: # Runs if not in windows 
    script: "ls {$@?}"

  windows:  # Other options are linux and macOS
    script: "dir {$@?}"
```

Os tasks can also be specified in a single key, i.e. the following is equivalent to the example above.

```yaml
tasks:
  ls: 
    script: "ls {$@?}"

  ls.windows:
    script: "dir {$@?}"
```


<a name="working-directory"></a>
### Working Directory
By default, the working directory of the task is one where it was executed. This can be changed at the task level
or root level, with `wd`. The path can be relative or absolute, with relative paths being resolved against the
configuration file and not the directory where the task was executed, this means `""` can be used to make the
working directory the same one as the directory for the configuration file.


<a name="task-inheritance"></a>
### Task Inheritance
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
- `args_extend` (added to `args` when parsing the child task,
 so the parent task would actually inherit `args`)
- `args+` (alias for `args_extend`)
- `private`

The inheritance works from bottom to top, with childs being processed before the parents. Circular dependencies
are not allowed and will result in an error.

It will attempt to find and the **os-specific** task first and inherit from it, if not found, it will use the regular task.
For example:

```yaml
tasks:
  sample.windows:
    script: "echo hello"
  
  sample:
    script: "echo hi"
  
  inherit:
    bases: [sample]
```

Is equivalent to:

```yaml
tasks:
  sample.windows:
    script: "echo hello windows"
  
  sample:
    script: "echo unix"
  
  inherit:
   script: "echo hello windows"
```

This way base tasks can be defined for each OS, and have only one version for its children.


<a name="extending-program-arguments"></a>
#### Extending Program Arguments

Args can be extended with `args_extend` or it's alias `args+`. These will append the given list to the `args`
inherited from the bases.

Examples:
```yaml
tasks:
  program:
    program: "program"
    args: ["{name}"]

  program_extend:
    bases: ["program"]
    args_extend: ["{phone}"]

  other:
    env: {"KEY": "VAL"}
    args: ["{other_param}"]
    private: true  # cannot be called directly, field not inherited

  program_extend_again:
    bases: ["program_extend", "other"]
    args+: ["{address}"]  # args+ is an alias for args_extend
```

In the example above, `program_extend_again` will be equivalent to
```yaml
tasks:
  program_extend_again:
    program: "program"
    env: {"KEY": "VAL"}
    args: ["{name}", "{phone}", "{address}"]
```


<a name="private-tasks"></a>
#### Private Tasks

Tasks can be marked as private by setting `private = true`. Private tasks cannot be called by the user.

<a name="Contributing"></a>
## Contributing
Feel free to create issues to report bugs, ask questions or request changes.

You can also fork the repository to make pull requests, just make sure the code is well tested.
Signed commits are preferred.