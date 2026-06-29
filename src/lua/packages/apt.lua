dots.provider.package("apt", {
	available = "command -v apt-get >/dev/null && command -v dpkg-query >/dev/null",
	installed = "dpkg-query -W -f='${Status}' \"$DOTS_PACKAGE\" 2>/dev/null | grep -q '^install ok installed$'",
	install = 'sudo apt-get install -y "$DOTS_PACKAGE"',
	remove = 'sudo apt-get remove -y "$DOTS_PACKAGE"',
})
