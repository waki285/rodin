// Global defaults applied to every article.
// Override per-post by adding your own #set later in the file.

#set figure(numbering: none)
#set raw(theme: "github-dark.tmTheme")
#show math.equation: it => context {
  // HTML 出力のときだけ特別扱い
  if target() == "html" {
    // インライン数式は box で包んで段落を分断しないようにする
    show: if it.block { it => it } else { box }
    html.frame(it) // 通常レイアウトで組んで SVG として埋め込む
  } else {
    it // PDF などその他のターゲットはそのまま
  }
}

// quote に attribution を指定したとき HTML 出力で落ちるので、
// attribution がある場合だけ独自レンダリングに差し替える。
#show quote: it => {
  if it.attribution == none {
    it
  } else {
    block(
      inset: (x: 0.9em, y: 0.35em),
      [
        #set text(style: "italic")
        #it.body
        #par(emph(text(weight: "semibold")[— #it.attribution]))
      ],
    )
  }
}

#show link: it => {
  // PDF 等の「paged」ターゲットでは何もしない
  if target() != "html" {
    it
  } else if type(it.dest) == str and it.dest.starts-with("http") {
    // URL 文字列のリンクだけを対象にする
    html.elem("a", attrs: (
      href: it.dest,
      target: "_blank",
      rel: "noopener noreferrer",
    ))[#it.body]
  } else {
    // 内部リンクなどはそのまま
    it
  }
}


// Inline SVG icons for callouts (HTML target only)
#let callout-icon(kind) = {
  let common = (
    class: "callout-svg",
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    xmlns: "http://www.w3.org/2000/svg",
    stroke-width: "2",
    stroke-linecap: "round",
    stroke-linejoin: "round",
    "aria-hidden": "true",
  )

  if kind == "warn" {
    html.elem("svg", attrs: common)[
      #html.elem("path", attrs: (
        d: "M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0",
      ))[]
      #html.elem("line", attrs: (x1: "12", y1: "9", x2: "12", y2: "13"))[]
      #html.elem("line", attrs: (x1: "12", y1: "17", x2: "12.01", y2: "17"))[]
    ]
  } else {
    html.elem("svg", attrs: common)[
      #html.elem("circle", attrs: (cx: "12", cy: "12", r: "10"))[]
      #html.elem("line", attrs: (x1: "12", y1: "16", x2: "12", y2: "12"))[]
      #html.elem("line", attrs: (x1: "12", y1: "8", x2: "12.01", y2: "8"))[]
    ]
  }
}

#let callout(body, kind: "info") = context {
  if target() == "html" {
    html.elem(
      "div",
      attrs: (class: "callout callout-" + kind),
    )[
      #html.elem("span", attrs: (class: "callout-icon"))[ #callout-icon(kind) ]
      #html.elem("div", attrs: (class: "callout-body"))[ #body ]
    ]
  } else {
    // PDF 用レイアウト（ここは好きなように）
    block(
      fill: rgb("#fffbeb"),
      inset: 12pt,
    )[body]
  }
}

#let toc() = context {
  outline(
    depth: 3,
    title: "目次"
  )
}

#let hr() = context {
  if target() == "html" {
    // HTML のときだけ <hr> を出す
    html.elem("hr")
  } else {
    // PDF / PNG / SVG では普通に線を引く
    line(length: 100%)
  }
}

#let twitter-link = "https://x.com/suzuneu_discord"