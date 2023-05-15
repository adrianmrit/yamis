# Yamis
![build](https://github.com/adrianmrit/yamis/actions/workflows/test.yml/badge.svg)
[![codecov](https://codecov.io/gh/adrianmrit/yamis/branch/main/graph/badge.svg?token=3BBJFNNJPT)](https://codecov.io/gh/adrianmrit/yamis)
![License: GPL v3](https://img.shields.io/github/license/adrianmrit/yamis)

> Task runner for teams and individuals. Written in [Rust](https://www.rust-lang.org/).

## Index
* [Inspiration](#inspiration)
* [Installation](#installation)
  * [Binary releases](#binary-releases)
  * [Updates](#updates)
* [Quick start](#quick-start)
* [Usage](#usage)
  * [Command line options](#command-line-options)
  * [Task files](#task-files)
  * [Common Properties](#common-properties)
    * [wd](#wd)
    * [env](#env)
    * [env_file](#env_file)
  * [Tasks File Properties](#tasks-file-properties)
    * [version](#version)
    * [tasks](#tasks)
  * [Task Properties](#task-properties)
    * [help](#help): The help message.
    * [bases](#bases): The bases to execute.
    * [script_runner](#script_runner)
    * [script_extension](#script_extension)
    * [script_ext](#script_ext)
    * [script](#script)
    * [cmds](#cmds)
    * [program](#program)
    * [args](#args)
    * [args_extend](#args_extend)
    * [args+](#args_extend)
    * [linux](#os-specific-tasks)
    * [windows](#os-specific-tasks)
    * [mac](#os-specific-tasks)
    * [private](#private)
  * [OS specific tasks](#os-specific-tasks)
  * [Passing arguments](#passing-arguments)
* [Contributing](#contributing)


<a name="inspiration"></a>
## Inspiration

Inspired by different tools like [cargo-make](https://github.com/sagiegurari/cargo-make),
[go-task](https://taskfile.dev/)
[doskey](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/doskey),
[bash](https://www.gnu.org/savannah-checkouts/gnu/bash/manual/bash.html)
and
[docker-compose](https://docs.docker.com/compose/).


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

When running, if a new version is available, a message will be displayed with the command. The update can be performed
by running `yamis --update`, which will download and replace the binary. Alternatively it can be updated by following
the installation instructions again at [Installation](#installation) or [Binary releases](#binary-releases).

Note that the program will cache the update information for 24 hours, so no need to panic about
it performing a request every time you run it.


<a name="quick-start"></a>
## Quick start

Create a file named `yamis.root.yml` in the root of your project.

Here is a very basic example of a task file:
```yaml
# yamis.root.yml
version: 2

vars:
  greeting: Hello World

tasks:
  hi:
    cmds:
      - echo {{ greeting }}
  
  hi.windows:
    script: echo {{ greeting }} from Windows
  
  sum:
    cmds:
      - echo "{{ args.0 }} + {{ args.1 }} = {{ args.0 | int + args.1 | int }}"
```

After having a config file, you can run a task by calling `yamis`, the name of the task, and any arguments, i.e.
`yamis hi`. Arguments can be passed right after the task name, either by name or position, i.e. `yamis sum 1 2`.


<a name="usage"></a>
## Usage

<a name="command-line-options"></a>
### Command line options
You can see some help about the command line options by running `yamis -h` or `yamis --help`. Essentially, the
usage would be like this:

```
Usage: yamis [OPTIONS] [COMMAND]

Options:
  -l, --list              Lists configuration files that can be reached from the current directory
  -t, --list-tasks        Lists tasks
  -i, --task-info <TASK>  Displays information about the given task
      --dry               Runs the task in dry mode, i.e. without executing any commands
  -f, --file <FILE>       Search for tasks in the given file
  -g, --global            Search for tasks in ~/yamis/yamis.global.{yml,yaml}
      --update            Checks for updates and updates the binary if necessary
  -h, --help              Print help
  -V, --version           Print version
```


<a name="task-files"></a>
### Task files

The tasks are defined using the YAML format.

When invoking a task, starting in the working directory and continuing to the root directory, the program will
look configuration files in a certain order until either a task is found, a `yamis.root.{yml,yaml}` file is found,
or there are no more parent folders (reached root directory). The name of these files is case-sensitive in case-sensitive
systems, i.e. `yamis.root.yml` will not work in linux.

The priority order is as follows:
- `yamis.private.yml`: Should hold private tasks and should not be committed to the repository.
- `yamis.private.yaml`: Same as above but for yaml format.
- `yamis.yml`: Should be used in sub-folders of a project for tasks specific to that folder and sub-folders.
- `yamis.yaml`: Same as above but for yaml format.
- `yamis.root.yml`: Should hold tasks for the entire project.
- `yamis.root.yaml`: Same as above but for yaml format.

An especial task file can be defined at `~/yamis/yamis.global.yml` or `~/yamis/yamis.global.yaml` for global tasks.
To run a global task, you need to pass the `--global` or `-g` flag, i.e. `yamis -g say_hi`. This is useful for
personal tasks that are not related to a specific project.

Tasks can also be defined in a different file by passing the `--file` or `-f` flag, i.e. `yamis -f my_tasks.yml say_hi`.

While you can add any of the two formats, i.e. `yamis.root.yml` and `yamis.root.yaml`, it is recommended to use
only one format for consistency and to avoid confusion.


<a name="common-properties"></a>
### Common Properties
The following properties can be defined in the task file or in the task itself. The value defined in the task takes
precedence over the value defined in the file.

- [wd](#wd): The default working directory.
- [env](#env): Environment variables.
- [env_file](#env_file): File containing environment variables.


<a name="wd"></a>
##### wd

The `wd` property is used to define the default working directory for the tasks in the file. The value of the
property is a string containing the path to the working directory. The path can be absolute or relative to the
location of the file.

If not defined in the file or task, it defaults to the directory where the command was
executed. To set the working directory relative to the location of the file, use `wd: ""`. Note that
`wd: "/"` will not work, as it will be interpreted as an absolute path.

The value defined in the executed task takes precedence over the value defined in the file.


<a name="env"></a>
##### env

The `env` property is used to define environment variables that will be available to all tasks in the file.
The value of the property is a map of key-value pairs, where the key is the name of the environment variable,
and the value is the value of the environment variable.

The value defined in the executed task takes precedence over the value defined in the file.


<a name="env_file"></a>
##### env_file

The `env_file` property is used to define environment variables that will be available to all tasks in the file.
The value of the property is a string containing the path to the file containing the environment variables.
The path can be absolute or relative to the location of the file.

The value defined in the executed task takes precedence over the value defined in the file. Also, the values
defined in the `env` property take precedence over the values defined in the file.


<a name="vars"></a>
##### vars

The `vars` property is used to define variables that will be available to all tasks in the file.
This behaves like the [env](#env) property, but the variables are not exported to the environment,
and can be more complex than strings.

For example, you can define a variable like this:

```yaml
vars:
  user:
    age: 20
    name: John
```

And then use it in a task like this:

```yaml
tasks:
  say_hi:
    cmd: echo "Hi, {{ user.name }}!"
```


<a name="tasks-file-properties"></a>
### Tasks File Properties

Besides the [common properties](#common-properties), the following properties can be defined in the task file:
- [tasks](#tasks): The tasks defined in the file.
- version: The version of the file. Although not used at the moment, it is required for future compatibility. The version
  can be a number or string. At the moment backward compatibility with version 1 was not implemented. Therefore, at the
  moment of writing this, the version should be `2` or `v2`.


<a name="tasks"></a>
##### tasks
The `tasks` property is used to define the tasks in the file. The value of the property is a map of key-value
pairs, where the key is the name of the task, and the value is the task definition.

The name of the task can be any string, but it is recommended to use only alphanumeric characters and dashes.
Private tasks should start with an underscore, i.e. `_private-task`.


<a name="task-properties"></a>
### Task Properties

Besides the common properties, the task can have the following properties:
- [help](#help): The help message.
- [bases](#bases): The bases to execute.
- [script_runner](#script_runner): A template to parse the script program and arguments.
- [script_extension](#script_extension): The extension of the script file.
- [script_ext](#script_ext): Alias for `script_extension`.
- [script](#script): The script to execute.
- [cmds](#cmds): The commands to execute.
- [program](#program): The program to execute.
- [args](#args): The arguments to pass to the program.
- [args_extend](#args_extend): The arguments to pass to the program, appended to the arguments from the base task, if any.
- [args+](#args_extend): Alias for `args_extend`.
- [linux](#os-specific-tasks): A version of the task to execute in linux.
- [windows](#os-specific-tasks): A version of the task to execute in windows.
- [mac](#os-specific-tasks): A version of the task to execute in mac.
- [private](#private): Whether the task is private or not.


<a name="help"></a>
##### help

The `help` property is used to define the help message for the task. The value of the property is a string
containing the help message.

Unlike comments, help will be printed when running `yamis -i <TASK>`.


<a name="bases"></a>
##### bases

The `bases` property is used to define the tasks to inherit from.

The inherited values are:
- `wd`
- `help`
- `script_runner`
- `script_extension`
- `script_ext` (alias for `script_extension`)
- `script`
- `program`
- `args`
- `cmds`

Values merged (with the task values taking precedence) are:
- `env`
- `env_file` (loaded and merged with the inherited `env` and `env_file`)

Values not inherited are:
- `args_extend` (appended to the inherited `args`)
- `args+` (alias for `args_extend`)
- `private`


<a name="script_runner"></a>
##### script_runner

The `script_runner` property is used to define the template to parse the script program and arguments. Must contain
a program and a `{{ script_path }}` template, i.e. `python {{ script_path }}`. Arguments are separated in the same way
as [args](#args).


<a name="script_extension"></a>
##### script_extension

The `script_extension` property is used to define the extension of the script file. I.e. `py` or `.py` for python scripts.


<a name="script"></a>
#### Script

**⚠️Warning:**
DO NOT PASS SENSITIVE INFORMATION AS PARAMETERS IN SCRIPTS. Scripts are stored in a file in the temporal
directory of the system and is the job of the OS to delete it, however it is not guaranteed that when or if that would
be the case. So any sensitive argument passed could be persisted indefinitely.

The `script` value inside a task will be executed in the command line (defaults to cmd in Windows
and bash in Unix). Scripts can spawn multiple lines, and contain shell built-ins and programs.

The generated scripts are stored in the temporal directory, and the filename will be a hash so that if the
script was previously called with the same parameters, we can reuse the previous file, essentially working
as a cache.

<a name="program"></a>
#### Program

The `program` value inside a task will be executed as a separate process, with the arguments passed
on `args`, if any.

<a name="args"></a>
#### Args

The `args` values inside a task will be passed as arguments to the program, if any. The value is a string
containing the arguments separated by spaces. Values with spaces can be quoted to be treated as one, i.e.
`"hello world"`. Quotes can be escaped with a backslash, i.e. `\"`.

<a name="args_extend"></a>
#### Args Extend
The `args_extend` values will be appended to `args` (with a space in between), if any. The value is a string
in the same form as `args`.

<a name="cmds"></a>
#### Cmds

The `cmds` value is a list of commands to execute. Each command can be either a string, or a map with a `task` key.

If the command is a string, it will be executed as a program, with the first value being the program, and the
rest being the arguments. Arguments are separated in the same way as [args](#args).

If the command is a map, the value of `task` can be either the name of a task to execute, or the definition of a
task to execute.

Example:
```yaml
tasks:
  say_hi:
    script: echo "hi"

  say_bye:
    script: echo "bye"
  
  greet:
    cmds:
      - python -c "print('hello')"
      - task: say_hi
      - task:
          bases: [say_bye]
```

<a name="private"></a>
#### Private
The `private` value is a boolean that indicates if the task is private or not. Private tasks cannot be executed
directly, but can be inherited from.


<a name="os-specific-tasks"></a>
### OS specific tasks

You can have a different OS version for each task. If a task for the current OS is not found, it will
fall back to the non os-specific task if it exists. I.e.
```yaml
tasks:
  ls:
    script: "ls {{ args.0 }}"

  ls.windows:
    script: "dir {{ args.0 }}"
```

Os tasks can also be specified in a single key, i.e. the following is equivalent to the example above.

```yaml
tasks:
  ls: 
    script: "ls {{ args.0 }}"

  ls.windows:
    script: "dir {{ args.0 }}"
```

Note that os-specific tasks do not inherit from the non-os specific task implicitly, if you want to do so, you will have
to define bases explicitly, i.e.

```yaml
tasks:
  ls:
    env:
      DIR: "."
    script: "ls {{ env.DIR }}"

  ls.windows:
    bases: [ls]
    script: "dir {{ env.DIR }}"
```


<a name="passing-arguments"></a>
### Passing arguments

Arguments for tasks can be either passed as a key-value pair, i.e. `--name "John Doe"`, or as a positional argument, i.e.
`"John Doe"`.

Named arguments must start with one or two dashes, followed by an ascii alpha character or underscore, followed by any number
of letters, digits, `-` or `_`. The value will be either the next argument or the value after the equals sign, i.e.
`--name "John Doe"`, `--name-person1="John Doe"`, `-name_person1 John` are all valid. Note that `"--name John"` is not
a named argument because it is surrounded by quotes and contains a space, however `"--name=John"` is valid named argument.

The first versions used a custom parser, but it takes a lot of work to maintain and it is not as powerful.
So now the template engine used is [Tera](https://tera.netlify.app/docs/). The syntax is
based on Jinja2 and Django templates. The syntax is very easy and powerful.

The exported variables are:
- `args`: The arguments passed to the task. If the task is called with `yamis say_hi arg1 --name "John"`, then
  `args` will be `["arg1", "--name", "John"]`.
- `kwargs`: The keyword arguments passed to the task. If the task is called with `yamis say_hi --name "John"`,
  then `kwargs` will be `{"name": "John"}`. If the same named argument is passed multiple times, the value will be
  the last one.
- `pkwargs`: Same as `kwargs`, but the value is a list of all the values passed for the same named argument.
- `env`: The [environment variables](#env) defined in the task. Note that this does not includes the environment variables
  defined in the system. To access those, use `{{ get_env(name=<value>, default=<default>) }}`.
- `vars`: The [variables](#vars) defined in the task.
- `TASK`: The [task](#task-properties) object and its properties.
- `FILE`: The [file](#tasks-file-properties) object and its properties.

Named arguments are also treated as positional arguments, i.e. if `--name John --surname=Doe` is passed,
`{{ args.0 }}` will be `--name`, `{{ args.1 }}` will be `John`, and `{{ args.2 }}` will be `--surname="Doe"`.
Thus, it is recommended to pass positional arguments first.

In you want to pass all the command line arguments, you can use `{{ args | join(sep=" ") }}`, or `{% for arg in args %} "{{ arg }}" {% %}`
if you want to quote them.

You can check the [Tera documentation](https://tera.netlify.app/docs/#introduction) for more information. Just ignore the Rust specific parts.


<a name="Contributing"></a>
## Contributing
Contributions welcome! Please read the [contributing guidelines](CONTRIBUTING.md) first.
