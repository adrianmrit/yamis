# Yamis
![build](https://github.com/adrianmrit/yamis/actions/workflows/test.yml/badge.svg)
![License: GPL v3](https://img.shields.io/github/license/adrianmrit/yamis)

> Task runner for teams and individuals. Written in [Rust](https://www.rust-lang.org/).

## Index
* [Motivation](#motivation)
* [Backward compatibility](#backward-compatibility)
* [Installation](#installation)
  * [Binary releases](#binary-releases)
  * [Updates](#updates)
* [Quick start](#quick-start)
* [Note about YAML and TOML files](#note-about-yaml-and-toml-files)
* [Usage](#usage)
  * [Command line options](#command-line-options)
  * [Task files](#task-files)
  * [Script](#script)
    * [Auto quoting](#auto-quoting)
    * [Replacing the runner](#replacing-the-runner)
  * [Program](#program)
  * [Running tasks serially](#running-tasks-serially)
  * [Script vs Program](#script-vs-program)
  * [Task arguments in the command line](#task-arguments-in-the-command-line)
  * [Tags](#tags)
  * [Expressions](#expressions)
    * [Positional parameters](#positional-parameters)
    * [Named parameters](#named-parameters)
    * [All parameters](#all-parameters)
    * [Environment variables](#environment-variables)
    * [String parameters](#string-parameters)
    * [Format strings](#format-strings)
    * [Functions](#functions)
  * [Optional expressions](#optional-expressions)
  * [Index and slice](#index-and-slice)
  * [Unpacking](#unpacking)
  * [Setting environment variables](#setting-environment-variables)
  * [OS specific tasks](#os-specific-tasks)
  * [Working directory](#working-directory)
  * [Documenting tasks](#documenting-tasks)
  * [Task inheritance](#task-inheritance)
    * [Extending program arguments](#extending-program-arguments)
    * [Private tasks](#private-tasks)
  * [List of functions](#list-of-functions)
    * [map](#map-function)
    * [join](#join-function)
    * [jmap](#jmap-function)
    * [fmt](#fmt-function)
    * [trim](#trim-function)
    * [split](#split-function)
* [Contributing](#contributing)

<a name="motivation"></a>
## Motivation
This project started out of necessity and fun as I wanted to learn Rust, but have become.
my everyday tool for running tasks. It aims to be simple and powerful, both for individuals and teams,
specially those working on different platforms. To allow for future improvements in the syntax while
not breaking the workflow of teams, it aims to be [backward compatible](#backward-compatibility), both between
the same mayor release, and with previous mayor releases.

Inspired on different tools like [cargo-make](https://github.com/sagiegurari/cargo-make),
[doskey](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/doskey),
[bash](https://www.gnu.org/savannah-checkouts/gnu/bash/manual/bash.html)
and
[docker-compose](https://docs.docker.com/compose/).


<a name="backward-compatibility"></a>
## Backward compatibility
Starting from version 1.0.0, the goal is to be backward compatible with mayor versions and follow
[Semantic Versioning](https://semver.org/).
When a mayor version is released, the plan is to support config files from the latest versions by setting
the mayor version in the TOML or YAML file with the `version` key, i.e. `version: "1"`.
If the version is not set, the least mayor version supported will be used.

<a name="installation"></a>
## Installation
If you have [Rust](https://www.rust-lang.org/) and [Cargo](https://doc.rust-lang.org/cargo/) installed ([rust installation instructions](https://www.rust-lang.org/tools/install)). Then run:
```bash
cargo install --force yamis
```

Pro-tip: make sure `~/.cargo/bin` directory is in your `PATH` environment variable.

<a name="binary-releases"></a>
### Binary releases:
Binaries are also available for Windows, Linux and macOS under
[releases](https://github.com/adrianmrit/yamis/releases/). To install, download the zip for your system, extract,
and copy the binary to the desired location. You will need to ensure the folder that contains the binary is available
in the `PATH`.


<a name="updates"></a>
### Updates
Automatic updates are not supported, but new releases will be notified when invoking the program. To achieve this it
will look into the repository for new releases, so it needs to be connected to the internet. The message will be cached
to avoid calling the server unnecessarily.

You can run `cargo install --force yamis` or download the latest binaries to update to the latest version.


<a name="quick-start"></a>
## Quick start
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
    help: "Just say hi"  # help message, printend when running `yamis -i say_hi`
    script: "echo Hello {name}"  # takes a name argument, i.e. `--name John`
  
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
`yamis say_hi --name "world"`. Passing the same argument multiple times will also add it multiple times, i.e.
`yamis say_hi --name "person 1" --name="person 2"` is equivalent to `echo Hello person 1 person 2`


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

<a name="command-line-options"></a>
### Command line options
You can see some help about the command line options by running `yamis -h` or `yamis --help`. Essentially, the
usage would be like this:

```
USAGE:
    yamis [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -f, --file <FILE>              Search for tasks in the given file
    -h, --help                     Print help information
    -i, --task-info <TASK>         Displays information about the given task
    -l, --list                     Lists configuration files that can be reached from the current
                                   directory
    -t, --list-tasks               Lists tasks
    -V, --version                  Print version information
```

You can either call a task directly by passing the name of the task and its arguments, i.e. `yamis say_hi --name John`,
or you can specify the configuration file to use with the -f option, i.e. `yamis -f project.yamis.yaml say_hi --name John`.
Note that the -f option is set before the task name, otherwise it would be interpreted as an argument for the task.

The next sections talks about how task files are auto-discovered.

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

If the task is still not found, it will look at `~/.yamis/user.yamis.toml` or `~/.yamis/user.yamis.yaml` or
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

<a name="replacing-the-runner"></a>
#### Replacing the runner
By default, the script runner in windows is CMD, and bash in unix systems. To use another program you can
set the `script_runner` option in a task, which should be a list, where the first value is the name of the program,
and the rest are extra parameters to pass before the script file parameter.

You might also want to override the `script_ext` (or `script_extension`) option, which is a string containing the
extension for the script file, and can be prepended with a dot or not. For some interpreter the extension does not
matter, but for others it does. In windows the extension defaults to `cmd`, and `sh` in unix.

Example:
```toml
# Python script that prints the date and time
[tasks.hello_world]
script_runner = ["python"]
script_ext = "py"  # or .py
script = """
from datetime import datetime

print(datetime.now())
"""
```

If using this feature frequently it would be useful to use inheritance to shorten the task. The above can become:
```toml
[tasks._py_script]
script_runner = ["python"]
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


<a name="task-arguments-in-the-command-line"></a>
### Task arguments in the command line
Arguments for tasks can be either passed as a key-value pair, i.e. `--name "John Doe"`, or as a positional argument, i.e.
`"John Doe"`.

Named arguments must start with one or two dashes, followed by an ascii alpha character or underscore, followed by any number
of letters, digits, `-` or `_`. The value will be either the next argument or the value after the equals sign, i.e.
`--name "John Doe"`, `--name-person1="John Doe"`, `-name_person1 John` are all valid. Note that `"--name John"` is not
a named argument because it is surrounded by quotes and contains a space, however `"--name=John"` is valid named argument.

Named arguments are also treated as positional arguments, i.e. if `--name John --surname=Doe` is passed,
`$1` will be `--name`, `$2` will be `John`, and `$3` will be `--surname="Doe"`. Thus, it is recommended to pass positional
arguments first.

In you want to pass the arguments as they are to a program, it doesn't matter how they are formatted, you can use the `{$@}`
tag, which will expand to all the arguments.

You can read more about the usage of arguments in tasks in the [Tags](#tags) and [Expressions](#expressions) sections.


<a name="tags"></a>
### Tags
Tags are used to insert dynamic values into the scripts and arguments of program we want to call. Tags can be
used to insert positional and named arguments, environment variables (with a cross-platform syntax) and invoke
functions.

The expressions inside tags (including functions) can return either a string, or a list of strings.
These are in fact the only two data types that can be used directly in tags. Note that empty lists and lists
with a single string will not be coerced into a string to avoid ambiguity, check the [Expressions](#expressions)
section for more info.

The integer is only allowed when slicing, i.e. `{values[0]}` is valid, but `{1}` is not.

In the case of optional expressions, there is no null value, they will simply return an empty string/list. For example
`{$1?}` will return an empty string if `$1` is not passed.

Why aren't more data types supported, like integers? Because parsing makes sense only for returning the body of a script
or the arguments for a program, and both are always strings or list of strings. Furthermore, it would make more sense to call
an external script for more complex operations.

<a name="expressions"></a>
### Expressions

<a name="positional-parameters"></a>
#### Positional parameters
1-indexed, start with `$` and followed by a number, i.e. `{$1}`, `{$2}`. These return a single string, so slices of them
will return a substring.

<a name="named-parameters"></a>
#### Named parameters
Case-sensitive and passed by name, i.e. `{out}`, `{file}`, etc. Note that any dash before the argument
is removed, i.e. if `--file out.txt` is passed, `{file}` will accept it. You can see the
[Task arguments in the command line](#task-arguments-in-the-command-line) section for more info.

These will always return a list of strings, so an index slice will return a string, while a range slice will return
a subarray. I.e. `{ file[0][0] }` returns the first character of the first passed `file` argument, while `file[0]`
will return the first file argument.

<a name="all-parameters"></a>
#### All parameters
With `{ $@ }` a list of all arguments will be passed as they are. I.e. if calling a tasks with arguments
`hello -o file.txt -o=file2.txt`, it will return a list with `["hello", "-o", "file.txt", "-o=file2.txt"]`.
They can be accessed by index and sliced, i.e. `{ $@[0] }` and `{ $@[0..2] }` are valid. Can also be optional,
i.e. `{ $@? }`.


<a name="environment-variables"></a>
#### Environment variables
Prefixed with `$`, i.e. `{ $HOME }`, `{ $PATH }`, etc. These are represented as strings, so slices will return
a substring. Do not confuse with positional arguments, which are numeric, i.e. `$1`, or with the all parameters
syntax `$@`.

Note that with this syntax environment variables are loaded when the script or program arguments are parsed, unlike
the native syntax that will not work in arguments of programs, and in the case of scripts, will be loaded by the shell.
This is intentional to avoid ambiguities and can keep them separate, or use env variables with a different interpreter
like python.


<a name="string-parameters"></a>
#### String parameters
Strings are another type of valid expressions, but they are more relevant in the
function's context. Strings are defined by single or double quotes, cannot contain unescaped new lines.
I.e. `{ "\"hello\" \n 'world'" }` is a valid string. Strings can also be sliced, but this is side effect of trying
to keep the parser simple rather than a useful feature.


<a name="format-strings"></a>
#### Format strings
These are just regular strings that are treated specially in some functions. I.e. [fmt](#fmt-function) takes a format string
and multiple arguments. Each `%s` occurrence in the string will be replaced with an argument of the same index. Note that in
format strings `%` needs to be escaped with another `%`, i.e. `%%s` will be replaced with `%s`.

For example ```{ fmt("hello %s", $1) }``` will return `hello <first argument>`.


<a name="functions"></a>
#### Functions
For a list of available functions, check the [Functions](#functions) section.

Predefined functions can be used to transform arguments in different ways. They can take values and can be
nested. I.e. `{ join(" ", split(",", $1)) }` will split the first argument by `","`, and join them back with a space.

At the moment it is not possible to define custom functions as this would require either using an external language such as python,
an embedded language such as lua, or implementing a new programming language. One of the goals of this program is to have a simple
and clear syntax, so adding support for defining functions breaks this. In most cases where
complex operations need to be performed, it would be better and cleaner to have a separate script (i.e. bash or python) that performs
the desired operation and then call it from a task with the appropriate arguments. Still, new functions might be added in the future
to support flexible argument parsing operations. Feel free to request a new function by submitting a new issue in the repo.


<a name="optional-expressions"></a>
### Optional expressions
By default, expressions must return a non-empty string or non-empty array of strings, otherwise an error will be raised.
Expressions can be made optional by adding `?`, i.e. `{ $1? }`, `{ map("hello %s", person?)? }`, `{ $@? }`, `{ output? }`.


<a name="index-and-slice"></a>
### Index and slice
[Expressions](#expressions), including the output of [functions](#list-of-functions) can be sliced for more flexibility.
The slices are 0 indexed, and accept positive and negative indexes. The whole expression can be either mandatory
or optional, i.e. `exp[1][0]?` does not fail and returns nothing if `exp` is not set or `exp[1]` is out of bounds, note that something like
`exp?[1]?[0]?` is invalid.

Here are some examples for parameters `hello world -p=1 -p=2 -p=3`:

| Expression                 | Result                               |
|----------------------------|--------------------------------------|
| `echo { $@[0] }`           | `echo hello`                         |
| `echo { $@[0][0] }`        | `echo h`                             |
| `echo { p[0] }`            | `echo 1`                             |
| `echo { p[:2] }`           | `echo 1 2`                           |
| `echo { p[1:] }`           | `echo 2 3`                           |
| `echo { $@[0:999] }`       | `echo hello world --p=1 --p=2 --p=3` |
| `echo { $@[:-1] }`         | `echo --p=3`                         |
| `echo { $@[-3:-1] }`       | `echo --p=1 --p=2`                   |
| `echo { $@[990:999][0]? }` | `echo `                              |
| `echo { $@[-999]? }`       | `echo `                              |

<a name="unpacking"></a>
### Unpacking
Expressions that return an array will be unpacked. For example, given the following tasks:

```toml
[tasks.say-hi]
script = "echo hello {person}"

[tasks.something]
program = "imaginary-program"
args = ["{ map('-o %s', f) }"]  # map returns an array of strings
```

If we call `yamis hello --person John1 --person John2`, it will run `echo hello John1 John2`.
Similarly, `yamis something -f out1.txt -f out2.txt` will call `imaginary-program` with
`["-o", "out1.txt", "-o", "out2.txt""]` parameters. Note that in the last case we call a [function](#functions)
called [map](#map-function).


<a name="setting-environment-variables"></a>
### Setting environment variables
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


<a name="os-specific-tasks"></a>
### OS specific tasks
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

Note that os-specific tasks do not inherit from the non-os specific task implicitly, if you want to do so, you will have
to define bases explicitly, i.e.

```yaml
tasks:
  ls:
    env:
      DIR: "."
    script: "ls {$DIR}"

  ls.windows:
    bases: [ls]
    script: "dir {$DIR}"
```


<a name="working-directory"></a>
### Working directory
By default, the working directory of the task is one where it was executed. This can be changed at the task level
or root level, with `wd`. The path can be relative or absolute, with relative paths being resolved against the
configuration file and not the directory where the task was executed, this means `""` can be used to make the
working directory the same one as the directory for the configuration file.


<a name="documenting-tasks"></a>
### Documenting tasks
Tasks can be documented using the `help` key. Unlike comments, help will be printed when running `yamis -i <TASK>`.
Note that help is inherited. If you wish to remove it, you can set it to `""`.


<a name="task-inheritance"></a>
### Task inheritance
A task can inherit from multiple tasks by adding a `bases` property, which should be a list names of tasks in
the same file. This works like class inheritance in common languages like Python, but not all values are 
inherited.

The inherited values are:
- `wd`
- `help`
- `quote`
- `script`
- `script_runner`
- `script_ext`
- `script_extension` (alias for `script_ext`)
- `program`
- `args`
- `serial`
- `env` (the values are merged instead of overwriting)
- `env_file` (the values are merged instead of overwriting)

Values not inherited are:
- `args_extend` (added to the inherited `args` and destroyed afterwards)
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

This way base tasks can be defined for each OS, and have only one version for its children. However, note that
os-specific tasks do not inherit implicitly from the non os-specif task. As in the above example, `sample.windows`
will not implicitly inherit from `sample`.


<a name="extending-program-arguments"></a>
#### Extending program arguments

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
#### Private tasks

Tasks can be marked as private by setting `private = true`. Private tasks cannot be called by the user, but are useful
for inheritance.


<a name="list-of-functions"></a>
### List of functions
List of predefined functions.

<a name="map-function"></a>
#### map function
**Signature:** `map<S: str | str[]>(fmt_string: str, values: S) -> S`

Maps each value to `fmt(fmt_string, val)`.

**Parameters:**
- `fmt_string`: [format string](#format-strings)
- `values`: Value or values to map

Example:
```yaml
sample:
  quote: never
  script: |
    echo {map("'%s'", $@)}


sample2:
  program: merge_txt_files
  args: ["{map('%s.txt', $@)}"]
```

`yamis sample person1 person2` will result in `echo hi 'person1' 'person2'`

`yamis sample2 file1 file2` will result in calling `merge_txt_files` with arguments `["file1.txt", "file2.txt"]`


<a name="join-function"></a>
#### join function
**Signature**: `join<S: str | str[]>(join_str: str, values: S) -> str`

The first parameter of `join` is a string that will be inserted between all values given in the second parameter
returning a single string. If the second parameter is a single string, it will be returned as is.

**Parameters:**
- `join_str`: String to insert between the values
- `values`: Value or values to join

Example:
```yaml
sample:
  quote: never
  script: |
    echo hello {join(" and ", $@)}
```

`yamis sample person1 person2` will result in `echo hi person1 and person2'`


<a name="jmap-function"></a>
#### jmap function
**Signature:** `jmap<S: str | str[]>(fmt_string: str, values: S) -> S`

Shortcut for `join("", map(fmt_string, values))`

**Parameters:**
- `fmt_string`: [format string](#format-strings)
- `values`: Value or values to map

Example:
```yaml
sample:
  quote: never
  script: |
    echo hi{jmap(" '%s'", $@)}


sample2:
  program: some_program
  args: ["{jmap('%s,', $@)}"]
```

`yamis sample person1 person2` will result in `echo hi 'person1' 'person2' `

`yamis sample2 arg1 arg2` will result in calling `some_program` with arguments `["arg1,arg2,"]`


<a name="fmt-function"></a>
#### fmt function
**Signature**: `fmt(fmt_string: str, *args: str) -> str`

The first parameter of `fmt` is a [format string](#format-strings), and the rest of the values are parameters to format
the string with. Note that those extra parameters must be string values, not list of strings, i.e. cannot pass directly
`$@`.

**Parameters:**
- `fmt_string`: [format string](#format-strings)
- `args`: Arguments that will replace the `%s` occurrence of the same index

Example:
```yaml
sample:
  quote: never
  script: |
    echo {fmt("Hi %s and %s", $1, $2)}
```

`yamis sample person1 person2` will result in `echo Hi person1 and person2`


<a name="trim-function"></a>
#### trim function
**Signature**: `trim<S: str | str[]>(value: S) -> S`

Removes leading and trailing whitespaces (including newlines) from the string or each string in list of strings.

**Parameters:**
- `value`: String or list of strings to trim

Example:
```yaml
sample:
  quote: never
  script: |
    echo {trim("  \n  hello world  \n")}
```

`yamis sample` will result in `echo hello world`



<a name="split-function"></a>
#### split function
**Signature**: `split(split_val: str, split_string: str) -> str`

Splits the string with the given value

**Parameters:**
- `split_val`: Value to split by
- `split_string`: String to split

Example:
```yaml
sample:
  quote: never
  script: |
    echo {split(",", "a,b,c")}
```

`yamis sample` will result in `echo a b c`

<a name="Contributing"></a>
## Contributing
Feel free to create issues to report bugs, ask questions or request changes.

You can also fork the repository to make pull requests, just make sure the code is well tested.
Signed commits are preferred.
