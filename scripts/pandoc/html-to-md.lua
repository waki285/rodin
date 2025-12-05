-- html-to-md.lua
-- HTML → Markdown 変換用の統合フィルター

local List = require 'pandoc.List'

-- 画像を alt テキストに置換
-- alt が無い場合は [画像] に置換
function Image(el)
  if #el.caption == 0 then
    return pandoc.Str("[画像]")
  else
    return el.caption
  end
end

-- data-lang 属性からコードブロックの言語を設定
-- <pre><code data-lang="js">...</code></pre> → ```js
function CodeBlock(cb)
  -- HTML の data-lang 属性から言語を取得
  local lang = cb.classes[1] or cb.attributes.lang or cb.attributes["data-lang"] or ""

  if lang then
    lang = lang:match("^%s*(.-)%s*$") or ""
  end

  -- GFM writer が空白を入れるバグを回避するため RawBlock で出力
  local fence = "```" .. lang .. "\n" .. cb.text .. "\n```"
  return pandoc.RawBlock('markdown', fence)
end

-- 水平線を --- に統一
function HorizontalRule(el)
  return pandoc.RawBlock('markdown', '---')
end

-- .prose ラッパーを剥がす
function Div(el)
  if el.classes:includes('prose') then
    return el.content
  end
end

-- リンクから余計な属性を削除して Markdown 記法に変換させる
-- target="_blank" や rel="noopener" などが付いていると HTML のまま出力される
function Link(el)
  -- 属性をすべてクリア
  el.attributes = {}
  el.classes = {}
  el.identifier = ""
  return el
end
