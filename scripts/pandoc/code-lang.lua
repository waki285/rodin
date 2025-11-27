-- code-lang-from-data.lua
-- <pre><code data-lang="js">...</code></pre> を ```js にする

function CodeBlock(cb)
  -- すでに言語クラスが付いている場合は何もしない
  if #cb.classes > 0 then
    return cb
  end

  -- HTML の data-lang="js" は attributes.lang で取れる想定
  local lang = cb.attributes.lang or cb.attributes["data-lang"]

  if lang and lang ~= "" then
    cb.classes:insert(1, lang)
  end

  return cb
end
