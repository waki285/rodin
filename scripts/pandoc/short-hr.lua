function HorizontalRule(el)
  -- GFM/Markdown 用の生ブロックに置き換える
  return pandoc.RawBlock('markdown', '---')
end