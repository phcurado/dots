dots.provider.package("pacman", {
	available = "command -v pacman >/dev/null",
	installed = 'pacman -Q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'sudo pacman -S --needed "$DOTS_PACKAGE"',
	remove = 'sudo pacman -Rns "$DOTS_PACKAGE"',
})
