## Motivation
Besides wanted to learn Rust, I always struggled finding
a task runner that had what I needed (such as good argument
parsing) and team oriented.

## Quick start

TODO: Add installation instructions

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
script = "python{( -c )1}"  # Runs either the python interpreter, or an inline program if given
```

After having a config file, you can run a task by calling `yamis`, the name of the task, and any arguments, i.e.
`yamis say_hello name="big world"`. Passing the same argument multiple times will also add it multiple times, i.e.
`yamis say_hello name="person 1" --name="person 2"` is equivalent to `echo Hello person 1 person 2`

### Adding prefix and suffix
You can add a prefix and suffix surrounded by parenthesis after and before the argument name inside the tag, i.e.
`{(-o )file?(.txt)}`, if `file=sample` is passed, it will add `-o sample.txt` to the script.

### Task files discovery
The program will look at the directory where it was invoked and its parents until a `project.yamis.toml` is
discovered or the root folder is reached. Valid filenames are the following:
 - `local.yamis.toml`: First one to look at for tasks. This one should hold private tasks and should not
 be committed to the repository.
 - `yamis.toml`: Second one to look at for tasks. Should be used in sub-folders of a project for tasks specific
 to that folder and sub-folders.
 - `project.yamis.toml`: Last one to look at for tasks. The file discovery stops when this one is found.

Note that you can have multiple `local.yamis.toml` and `yamis.toml` files in a project.