dots.provider.package("snap", {
	capability = "provider:snap",
	available = "command -v snap >/dev/null",
	installed = 'snap list "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'sudo snap install "$DOTS_PACKAGE"',
	remove = 'sudo snap remove "$DOTS_PACKAGE"',
	list = "snap list | awk 'NR > 1 { print $1 }'",
})
