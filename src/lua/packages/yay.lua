dots.provider.package("yay", {
	available = "command -v yay >/dev/null",
	installed = 'yay -Q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'yay -S --needed --noconfirm "$DOTS_PACKAGE"',
	remove = 'yay -Rns --noconfirm "$DOTS_PACKAGE"',
})

dots.yay.enable = function(opts)
	opts = opts or {}
	local method = opts.method or "aur"

	if method == "pacman" then
		dots.pacman.install({ "yay" })
		return
	end

	if method == "aur" then
		dots.pacman.install({ "base-devel", "git" })
		dots.command("yay", {
			check = "command -v yay >/dev/null",
			apply = [[
				tmp="$(mktemp -d)"
				git clone https://aur.archlinux.org/yay.git "$tmp"
				makepkg -Ccsir --noconfirm -D "$tmp"
				rm -rf "$tmp"
			]],
			needs = { "package:pacman:base-devel", "package:pacman:git" },
			provides = { "provider:yay" },
		})
		return
	end

	error("unsupported yay enable method: " .. method)
end
