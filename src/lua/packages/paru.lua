dots.provider.package("paru", {
	available = "command -v paru >/dev/null",
	installed = 'paru -Q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'paru -S --needed "$DOTS_PACKAGE"',
	remove = 'paru -Rns "$DOTS_PACKAGE"',
})
