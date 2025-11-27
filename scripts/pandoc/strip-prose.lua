-- strip-prose-div.lua
local List = require 'pandoc.List'

function Div(el)
  if el.classes:includes('prose') then
    -- wrapper を剥がして中身だけ返す
    return el.content
  end
end
