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

#let generate_og(title: "", author: "すずねーう", icon: "suzuneu.webp", date: none, updated: none, body) = {
  place(background)
  
  // Center card
  place(center + horizon, rect(
    width: 90%,
    height: 80%,
    fill: white,
    radius: 20pt,
    stroke: none,
    inset: 60pt,
  )[
    #set align(left + horizon)
    #set text(font: ("IBM Plex Sans JP", "Hiragino Kaku Gothic ProN", "Noto Sans JP", "Roboto"), size: 64pt, weight: "bold", fill: rgb("#333333"))
    
    // Title
    #block(width: 100%, spacing: 2em)[
      #title
    ]
    
    // Footer (Author info)
    #v(1fr)
    #stack(dir: ltr, spacing: 20pt,
      box(width: 80pt, height: 80pt, radius: 40pt, clip: true, image(icon, width: 100%, height: 100%)),
      align(horizon, text(size: 32pt, fill: rgb("#555555"), weight: "medium")[
        #author
      ]),
      h(1fr),
      align(horizon, text(size: 32pt, fill: rgb("#999999"), weight: "bold")[
        // Display dates if available
        #if updated != none {
            // Check if updated is different/present
             if updated != date {
               [Updated: #updated]
             } else {
               [Published: #date]
             }
        } else if date != none {
            [Published: #date]
        }
      ])
    )
  ])
}
