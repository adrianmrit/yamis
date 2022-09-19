# Yamis
![build](https://github.com/adrianmrit/yamis/actions/workflows/test.yml/badge.svg)
![License: GPL v3](https://img.shields.io/github/license/adrianmrit/yamis)

## Motivation
Although there are great similar tools out there, the ones I have tried lack some things
I was interested in, like good argument parsing, and team oriented configuration files.
For instance, some of these tools didn't let you pass extra arguments at all, while others were
limited to positional arguments or passing all arguments inline.


## Features
- Multiplatform (Windows, Linux and macOs)
- Write config files in either YAML and TOML format
- Separate configuration files for teams and local development
- Powerful argument parsing
- Run scripts or programs with arguments
- Set the interpreter for scripts
- Tasks can inherit from one or more tasks
- Pass additional named or positional arguments to the underlying command
- Add extra environment variables or load an environment file
- Define task for specific operating systems


## Install

If you already have rustc and cargo installer, the easiest way to install is with:
```bash
cargo install --force yamis
```

Compiled binaries are also available for Windows, Linux and macOS under
[releases](https://github.com/adrianmrit/yamis/releases/tag/v0.1.0).

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

  folder_content:  # Default for linux and macOS, can be individually specified like for windows.
    script: "ls {$1?}"  # Takes a single optional argument
    windows:  # Task version for windows systems
      script: "dir {$1?}"

  compose-run:
    wd: ""  # Uses the dir where the config file appears as working dir
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


## Type conversion in YAML and TOML files:
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

## Usage
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


### Script
The `script` value inside a task will be executed in the command line (defaults to cmd in Windows
and bash in Unix). Scripts can spawn multiple lines, and contain shell built-ins and programs. When
passing multiple arguments, they will be expanded by default, the common example would be the `"{ $@ }"`
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

Although quoting prevents common errors like things breaking because an argument with a space was passed,
it might fail in certain edge cases.


##### Replacing interpreter
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


### Program
The `program` value inside a task will be executed as a separate process, with the arguments passed
on `args`. Note that each argument can contain at most one tag, that is, `{$1}{$2}` is not valid. When
passing multiple values, they are unpacked into the program arguments, i.e. `"{$@}"` will result in
all arguments passed down to the program.

When using inheritance, the arguments for the base can be extended by using `args_extend` instead of `args`.
This is useful for adding extra parameters without rewriting them.


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


### Tags
Tags are used to insert dynamic values into the scripts and arguments of program we want to call. Tags can be
used to insert positional and named arguments, environment variables (with a cross-platform syntax) and invoke
functions.

The expressions inside tasks can return values either as a string, or as a list of strings. If no values are passed,
the value will be an empty list, or an empty string in the case of positional arguments. This is specially relevant
when slicing and invoking functions.

#### Slicing
Arguments (more on arguments below) can be sliced for more flexibility. The slices are 0 indexed, here are some examples:

```text
{ $@[0] }                         # same as { $1 }

{ $@[0..2] }                      # first two arguments

{ map(f"hello {}", name)[0..2] }  # same as { map(f"hello {}", name[0..2]) }

{ fmt(f"hello {}", $1)[0] }       # returns `h`

{ $1[0] }                         # returns first char of first argument

{ $@[0][0] }                      # also returns first char of first argument
```

### Type of parameters

#### Positional
1-indexed, start with `$` and followed by a number, i.e. `{$1}`, `{$2}`. Represent a single string, so slices of them
will return a substring.

#### Named
Case-sensitive and passed by name, i.e. `{out}`, `{file}`, etc. Note that any dash before the argument
is removed, i.e. if `--file=out.txt` is passed, `{file}` will accept it. Also note that the named argument passed
to the task will need to be in the form `<key>=<value>`, i.e. `-o out.txt` is not recognized as a named argument,
this is to prevent ambiguities as the parsing of arguments can change from application to application.

These are represented by arrays of strings, so an index slice will return a string, while a range slice will return
a subarray. I.e. `{ file[0][0] }` returns the first character of the first passed `file` argument, while `file[0]`
will return the first file argument.

#### All arguments
With `{ $@ }`, all arguments will be passed as they are. Can be treated as a named argument

### Valid named argument tags
Named argument tasks must start with an ascii alpha character or underscore, and should be followed by any number
of letters, digits, `-` or `_`.

### Optional expressions
By default, expressions must return a non-empty string or non-empty array of strings, otherwise an error will be raised.
Expressions can be made optional by adding `?`, i.e. `{ $1? }`, `{ map("hello {}", person?)? }`.

### Unpacking
Expressions that return an array will be unpacked. For example, given the following tasks:

```toml
[tasks.say-hi]
script = "echo hello {person}"

[tasks.something]
program = "imaginary-program"
args = ["{ map('-o {}', f) }"]
```

If we call `yamis hello person=John1 person=John2`, it will run `echo hello "John1" "John2"`.
Similarly, `yamis something --f=out1.txt out2.txt` will call `imaginary-program` with
`["-o out1.txt", "-o out2.txt""]`. You might have noticed we call a `map`, more on functions later.


### Environment variables
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
Environment variables can be passed in `args`, `args_extend` or `scripts` similar to argument tags, i.e. `{ $ENV_VAR }`
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
echo { $SAMPLE }
"""
```

### Strings
Strings are another type of valid expressions. Tags also accept plain strings, but they are more relevant in the
function's context. Strings are defined by single or double quotes, cannot contain unescaped new lines.
I.e. `"\"hello\" \n 'world'"` is a valid string. Strings can also be sliced, but this is a more side effect of trying
to keep the parser simple than a useful feature.

### Functions
Predefined functions can be used to transform arguments in different ways. They can take values and can be
nested.

Functions can take string or array values, and also return either a single string or an array.

#### map
**Signature:** `map(fmt_string: str, values: str[]) -> str[]`

Maps each value to `fmt(fmt_string, val)`, where `fmt` replaces `{}` with value. Note that brackets
can be escaped by duplicating them, i.e. `{{` will be replaced with `{`

**Parameters:**
- fmt_string: String to format, i.e. `"-o {}.txt"`
- values: Values to map

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


#### flat
**Signature:** `flat(fmt_string: str, values: str[]) -> str`

`flat` is similar to map, but in scripts extra spaces won't be added, and in arguments it will not be unpacked. This is
because calling `flat` is like calling `map` and joining the resulting array values into a single string.

**Parameters:**
- fmt_string: String to format, i.e. `"-o {}.txt"`
- values: Values to map

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


#### join
**Signature**: `join(join_str: str, values: str[]) -> str`

The first parameter of `join` is a string that will be inserted between all values given in the second parameter
returning a single string.

**Parameters:**
- join_str: String to insert between the values
- values: Values to join

Example:
```yaml
sample:
  quote: never
  script: |
    echo hello {flat(" and ", $@)}
```

`yamis sample person1 person2` will result in `echo hi person1 and person2'`

#### fmt
**Signature**: `fmt(fmt_string: str, ...args: str[]) -> str`

The first parameter of `fmt` is a format string, and the rest of the values are parameters to format the string with.
Note that those extra parameters must be i individual values, not arrays, i.e. cannot use `$@`.

**Parameters:**
- fmt_string: String to format, i.e. `"-o {}.txt"`
- args: Arguments that will replace the `{}` occurrence of the same index

Example:
```yaml
sample:
  quote: never
  script: |
    echo {fmt("Hi {} and {}", $1, $2)}
```

`yamis sample person1 person2` will result in `echo Hi person1 and person2`

### Os Specific Tasks
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

### Working directory
By default, the working directory of the task is one where it was executed. This can be changed at the task level
or root level, with `wd`. The path can be relative or absolute, with relative paths being resolved against the
configuration file and not the directory where the task was executed, this means `""` can be used to make the
working directory the same one as the directory for the configuration file.


### Task inheritance
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

#### Extending args

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

#### Marking a task as private

Tasks can be marked as private by setting `private = true`. Private tasks cannot be called by the user.

## Contributing
Feel free to create issues to report bugs, ask questions or request changes.

You can also fork the repository to make pull requests, just make sure the code is well tested.
Signed commits are preferred.