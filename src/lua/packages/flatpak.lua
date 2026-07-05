dots.provider.package("flatpak", {
	capability = "provider:flatpak",
	available = "command -v flatpak >/dev/null",
	installed = 'flatpak info "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'flatpak install --assumeyes --noninteractive "$DOTS_PACKAGE"',
	remove = 'flatpak uninstall --assumeyes --noninteractive "$DOTS_PACKAGE"',
	list = "flatpak list --app --columns=application",
})
