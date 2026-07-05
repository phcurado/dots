dots.provider.package("apk", {
	capability = "provider:apk",
	available = "command -v apk >/dev/null",
	installed = 'apk info -e "$DOTS_PACKAGE" 2>/dev/null | grep -qx "$DOTS_PACKAGE"',
	install = 'sudo apk add "$DOTS_PACKAGE"',
	remove = 'sudo apk del "$DOTS_PACKAGE"',
	list = "apk info",
})
