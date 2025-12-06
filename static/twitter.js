// Twitter embed loader for rodin
(() => {
  const SCRIPT_ATTR = "data-rodin-twitter-script";
  const LOADER_ATTR = "data-rodin-twitter-loader";
  const WIDGET_SRC = "https://platform.twitter.com/widgets.js";
  const THEME_KEY = "rodin-theme";

  const ttPolicy = (() => {
    const NAME = "rodin-twitter";
    const existing = window.trustedTypes?.getPolicy?.(NAME);
    if (existing) return existing;
    try {
      return window.trustedTypes?.createPolicy(NAME, {
        createScriptURL: (u) => u,
      });
    } catch (_) {
      return window.trustedTypes?.getPolicy?.(NAME) ?? null;
    }
  })();

  const currentTheme = () => {
    const stored = localStorage.getItem(THEME_KEY);
    if (stored === "dark" || stored === "light") return stored;
    return document.documentElement.classList.contains("dark") ? "dark" : "light";
  };

  const applyTheme = () => {
    const theme = currentTheme();
    document.querySelectorAll("blockquote.twitter-tweet").forEach((el) => {
      if (el.getAttribute("data-theme") !== theme) el.setAttribute("data-theme", theme);
    });
  };

  let widgetsPromise = null;
  const loadWidgetsScript = () => {
    if (window.twttr?.widgets) return Promise.resolve(window.twttr.widgets);
    if (widgetsPromise) return widgetsPromise;

    widgetsPromise = new Promise((resolve, reject) => {
      const existing = document.querySelector(`script[${SCRIPT_ATTR}="1"]`);
      if (existing && window.twttr?.widgets) {
        resolve(window.twttr.widgets);
        return;
      }

      const s = document.createElement("script");
      s.src = ttPolicy ? ttPolicy.createScriptURL(WIDGET_SRC) : WIDGET_SRC;
      s.async = true;
      s.setAttribute(SCRIPT_ATTR, "1");
      const nonce = document.currentScript?.nonce;
      if (nonce) s.nonce = nonce;
      s.referrerPolicy = "origin-when-cross-origin";
      s.onload = () => resolve(window.twttr?.widgets);
      s.onerror = reject;
      document.head.appendChild(s);
    });

    return widgetsPromise;
  };

  const hydrate = () => loadWidgetsScript().then((w) => w?.load());

  const scan = () => {
    applyTheme();
    const tweets = document.querySelectorAll("blockquote.twitter-tweet");
    if (tweets.length === 0) return;
    ensureIO(tweets);
  };

  let io = null;
  const ensureIO = (nodes) => {
    if (io) {
      nodes.forEach((n) => io.observe(n));
      return;
    }
    io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting || e.intersectionRatio > 0)) {
          hydrate();
          io?.disconnect();
          io = null;
        }
      },
      { rootMargin: "200px 0px" }
    );
    nodes.forEach((n) => io.observe(n));
  };

  const schedule = (() => {
    let ticking = false;
    return () => {
      if (ticking) return;
      ticking = true;
      requestAnimationFrame(() => {
        ticking = false;
        scan();
      });
    };
  })();

  const moBody = new MutationObserver(schedule);
  const moHtml = new MutationObserver(schedule);

  const startObservers = () => {
    moBody.observe(document.body, { childList: true, subtree: true });
    moHtml.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });
  };

  const init = () => {
    startObservers();
    scan();
  };

  if (document.readyState === "complete" || document.readyState === "interactive") {
    init();
  } else {
    document.addEventListener("DOMContentLoaded", init, { once: true });
  }

  // Re-run from SPA navigation if needed
  window.__rodinTwitterInit = schedule;
})();
