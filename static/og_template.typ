// 1200x630 is the standard OG image size
#set page(width: 1200pt, height: 630pt, margin: 0pt)

// Zenn-like gradient background
#let background = rect(
  width: 100%,
  height: 100%,
  fill: gradient.linear(
    angle: 45deg,
    rgb("#FF9A9E"),
    rgb("#FECFEF"),
    rgb("#99E1D9"),
    rgb("#99A8E1"),
  )
)

#let generate_og(title: "", author: "すずねーう", icon: "suzuneu.webp", date: none, updated: none, description: "", body) = {
  place(background)
  
  // Center card
  place(center + horizon, rect(
    width: 90%,
    height: 80%,
    fill: white,
    radius: 20pt,
    stroke: none,
    inset: (x: 60pt, y: 50pt),
  )[
    #set align(left + top)
    #set text(font: ("IBM Plex Sans JP"), fill: rgb("#333333"))
    
    // タイトルの長さに応じてフォントサイズを調整
    #let title-size = if title.len() > 40 {
      48pt
    } else if title.len() > 25 {
      56pt
    } else {
      64pt
    }
    
    // Title - 上に詰める
    #block(width: 100%, spacing: 0pt)[
      #text(size: title-size, weight: "bold")[#title]
    ]
    
    #v(30pt)

    // Description (if available)
    #block(width: 100%, spacing: 0pt)[
      #if description != "" {
        text(size: 28pt, weight: "regular", fill: rgb("#666666"))[#description]
      }
    ]
    
    // 余白
    #v(1fr)
    
    // Footer (Author info) - 固定位置
    #stack(dir: ltr, spacing: 20pt,
      box(width: 80pt, height: 80pt, radius: 40pt, clip: true, image(icon, width: 100%, height: 100%)),
      align(horizon, text(size: 32pt, fill: rgb("#555555"), weight: "medium")[
        #author
      ]),
      h(1fr),
      align(horizon, text(size: 28pt, fill: rgb("#999999"), weight: "bold")[
        // Display dates if available
        #if updated != none and updated != "" {
          // Check if updated is different/present
          if updated != date {
            [Updated: #updated]
          } else {
            [Published: #date]
          }
        } else if date != none and date != "" {
          [Published: #date]
        }
      ])
    )
  ])
}
