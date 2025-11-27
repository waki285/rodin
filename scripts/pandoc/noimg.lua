-- img-alt-to-text.lua
-- alt が無い（caption が空）の img は [画像] に、
-- alt があるものは alt テキストに置換する

function Image(el)
  -- el.caption は Inlines（alt テキスト相当）
  if #el.caption == 0 then
    -- alt が無い: [画像] に置換
    return pandoc.Str("[画像]")
  else
    -- alt がある: alt テキストそのものに置換
    -- （Span で包む必要はなく、Inlines を直接返してよい）
    return el.caption
  end
end
