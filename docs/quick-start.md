# Quick start

`dots` helps manage dotfiles declaratively across machines and environments. It
can work with an existing dotfiles repo, or with a new setup from scratch. It
can replace parts of a setup currently handled by Stow, package lists, service
commands, or shell scripts.

In this page we will create a small dotfiles setup. We will link a config file
from your dotfiles repo into your home directory, then add OS packages that
differ between Arch Linux and macOS.

## Install

Install the latest release:

```sh
curl -fsSL https://raw.githubusercontent.com/phcurado/dots/main/install.sh | sh
```

If you are working from a checkout of the `dots` source repo, install it with:

```sh
make install
```

## Create a dotfiles repo

Start in the repo that will hold your dotfiles:

```sh
mkdir dotfiles
cd dotfiles
```

If you already have a dotfiles repo, use that directory instead.

Initialize `dots` there:

```sh
dots init
```

That creates `dots.lua`:

```text
dotfiles/
  dots.lua
```

`dots.lua` is the entrypoint of your dotfiles configuration. It describes what
should exist on your machine.

## Link a config file

Some config files are worth tracking in a dotfiles repo so the same setup can
be used across different computers. Programs still read those files from fixed
paths in your home directory (`$HOME`), such as `~/.zshrc`, `~/.gitconfig`,
`~/.config/nvim`, and so on.

One of the first steps in managing dotfiles is creating symlinks from those
system paths back to your repo. Then another machine can use the same references
without copying files by hand.

For a shell config stored as `.zshrc` in the repo, add this to `dots.lua`:

```lua
dots.symlink("~/.zshrc", ".zshrc")
```

The first path is where the operating system expects the file. The second path
is the file in your dotfiles repo.

Run:

```sh
dots check
```

`dots check` reads the machine state declared in `dots.lua` and prints the diff.
If `~/.zshrc` is not managed yet, it shows:

```diff
Symlinks:
  + symlink ~/.zshrc -> .zshrc

Check: 1 to create, 0 to update, 0 to destroy.
```

If the symlink already points to `.zshrc`, the output is:

```text
No changes.
```

## Install packages

The same config can also manage OS packages. If different machines use different
package managers, keep that logic in Lua:

```lua
if dots.platform.family == "arch" then
  dots.paru.enable({ method = "pacman" })
  dots.paru.install({ "ripgrep" })
end

if dots.platform.family == "darwin" then
  dots.brew.enable()
  dots.brew.install({ "wget" })
end
```

On Arch Linux, `dots check` shows the Arch package:

```diff
Packages:
  + paru ripgrep
```

On macOS, the same config shows the Homebrew package:

```diff
Packages:
  + brew wget
```

## Apply

After inspecting the diff, apply the configuration:

```sh
dots apply
```

`dots apply` asks for confirmation before changing the machine:

```text
Type 'yes' to apply these changes.
Apply?
```

After applying, `dots` creates the symlink and installs the package for the
current operating system.
