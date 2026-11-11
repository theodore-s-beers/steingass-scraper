-- Each request is a byte length, newline, then an independent HTML fragment.
-- Responses use the same framing. Entries never share a Pandoc document.
-- Pandoc injects this global when it runs the script via `pandoc lua`.

---@diagnostic disable-next-line: undefined-global
local pandoc = pandoc

local options = pandoc.WriterOptions({ wrap_text = "none" })
while true do
	local length = io.read("*l")
	if not length then
		break
	end
	local size = assert(tonumber(length))
	local input = size == 0 and "" or assert(io.read(size))
	assert(#input == tonumber(length), "truncated HTML request")
	local output = pandoc.write(pandoc.read(input, "html"), "markdown_strict", options)
	io.write(tostring(#output), "\n", output)
	io.flush()
end
