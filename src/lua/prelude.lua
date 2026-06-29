dots.provider.package("pacman", {
	available = "command -v pacman >/dev/null",
	installed = 'pacman -Q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'sudo pacman -S --needed "$DOTS_PACKAGE"',
	remove = 'sudo pacman -Rns "$DOTS_PACKAGE"',
})

dots.provider.package("paru", {
	available = "command -v paru >/dev/null",
	installed = 'paru -Q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'paru -S --needed "$DOTS_PACKAGE"',
	remove = 'paru -Rns "$DOTS_PACKAGE"',
})

dots.provider.package("apt", {
	available = "command -v apt-get >/dev/null && command -v dpkg-query >/dev/null",
	installed = "dpkg-query -W -f='${Status}' \"$DOTS_PACKAGE\" 2>/dev/null | grep -q '^install ok installed$'",
	install = 'sudo apt-get install -y "$DOTS_PACKAGE"',
	remove = 'sudo apt-get remove -y "$DOTS_PACKAGE"',
})

dots.provider.package("brew", {
	available = "command -v brew >/dev/null",
	installed = 'brew list --formula "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'brew install "$DOTS_PACKAGE"',
	remove = 'brew uninstall "$DOTS_PACKAGE"',
})
