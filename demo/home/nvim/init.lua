-- The nvim the demo runs. Read through XDG_CONFIG_HOME=demo/home, so nothing of the
-- recorder's own nvim config is loaded.

vim.opt.number = true
vim.opt.termguicolors = true
vim.opt.laststatus = 0
vim.opt.ruler = false
vim.opt.showcmd = false
vim.opt.showmode = false
vim.opt.cmdheight = 0
vim.opt.fillchars = { eob = " " }

pcall(vim.cmd.colorscheme, "vanadis")
