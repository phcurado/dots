dots.provider.package("paru", {
	capability = "provider:paru",
	available = "command -v paru >/dev/null",
	installed = 'paru -Q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'paru -S --needed --noconfirm "$DOTS_PACKAGE"',
	remove = 'paru -Rns --noconfirm "$DOTS_PACKAGE"',
	list = "pacman -Qq",
})

dots.paru.enable = function(opts)
	opts = opts or {}
	local method = opts.method or "pacman"

	if method == "pacman" then
		dots.pacman.install({ "paru" })
		return
	end

	if method == "aur" then
		dots.command("paru", {
			check = "command -v paru >/dev/null",
			apply = [[
				tmp="$(mktemp -d)"
				git clone https://aur.archlinux.org/paru.git "$tmp"
				makepkg -Ccsir --noconfirm -D "$tmp"
				rm -rf "$tmp"
			]],
			provides = { "provider:paru" },
		})
		return
	end

	error("unsupported paru enable method: " .. method)
end
