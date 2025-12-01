// Global defaults applied to every article.
// Override per-post by adding your own #set later in the file.

#set figure(numbering: none)
#set raw(theme: "github-dark.tmTheme")
#show math.equation: it => context {
  if target() == "html" {
    // HTML で出ないのを直す
    show: if it.block { it => it } else { box }
    html.frame(it)
  } else {
    it
  }
}

// quote attribution が出ないのをなおす
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
  if target() != "html" {
    it
  } else if type(it.dest) == str and it.dest.starts-with("http") {
    html.elem("a", attrs: (
      href: it.dest,
      target: "_blank",
      rel: "noopener noreferrer",
    ))[#it.body]
  } else {
    it
  }
}

#let ghlink(..args) = context {
  let pos = args.pos()
  let named = args.named()

  let path = pos.at(0)
  let label = if pos.len() >= 2 { pos.at(1) } else { none }

  let repo = named.at("repo", default: "waki285/rodin")
  let branch = named.at("branch", default: "main")

  let normalized = if path.starts-with("/") { path.slice(1) } else { path }
  let url = "https://github.com/" + repo + "/blob/" + branch + "/" + normalized
  let display = if label == none { normalized } else { label }

  link(url)[#display]
}

#let img(src, alt: "", lazy: false, width: none, height: none) = context {
  if target() == "html" {
    html.elem("img", attrs: (
      src: src,
      alt: alt,
      width: if width != none { width } else { null },
      height: if height != none { height } else { null },
      loading: if lazy { "lazy" } else { "eager" },
    ))
  } else {
    image(src, alt: alt, width: width)
  }
}


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
    html.elem("hr")
  } else {
    line(length: 100%)
  }
}

#let twitter-link = "https://x.com/suzuneu_discord"