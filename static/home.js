// Home-only script: run behaviors that should exist only on "/".
(() => {
  if (window.location.pathname !== "/") return;
  const xIcons = document.querySelectorAll(".icon-x");
  const twitterIcons = document.querySelectorAll(".icon-twitter");
  window.addEventListener("keydown", (e) => {
    if (e.shiftKey) {
      xIcons.forEach((icon) => icon.classList.add("hidden"));
      twitterIcons.forEach((icon) => icon.classList.remove("hidden"));
    } else {
      xIcons.forEach((icon) => icon.classList.remove("hidden"));
      twitterIcons.forEach((icon) => icon.classList.add("hidden"));
    }
  });
  window.addEventListener("keyup", (e) => {
    if (!e.shiftKey) {
      xIcons.forEach((icon) => icon.classList.remove("hidden"));
      twitterIcons.forEach((icon) => icon.classList.add("hidden"));
    }
  });

  // Delay loading the heavy hero background until after LCP is settled.
  const loadDeferredBackground = () => {
    const picture = document.querySelector("[data-deferred-bg]");
    if (!picture || picture.dataset.loaded === "1") return;

    picture.querySelectorAll("source[data-srcset]").forEach((source) => {
      const set = source.getAttribute("data-srcset");
      if (set) {
        source.srcset = set;
        source.removeAttribute("data-srcset");
      }
    });

    const img = picture.querySelector("img[data-src]");
    if (img instanceof HTMLImageElement) {
      const sizes = img.getAttribute("data-sizes") || picture.getAttribute("data-bg-sizes");
      if (sizes) img.sizes = sizes;
      const srcset = img.getAttribute("data-srcset");
      if (srcset) img.srcset = srcset;
      const src = img.getAttribute("data-src");
      if (src) img.src = src;
      img.fetchPriority = "low";
      img.loading = "lazy";
    }

    picture.dataset.loaded = "1";
  };

  const deferBackgroundUntilAfterLCP = () => {
    // If the script loads after window.load (it does, because we defer import),
    // LCP is already recorded—just load immediately.
    if (document.readyState === "complete") {
      loadDeferredBackground();
      return;
    }

    let fired = false;
    const trigger = () => {
      if (fired) return;
      fired = true;
      if ("requestIdleCallback" in window) {
        requestIdleCallback(loadDeferredBackground, { timeout: 1500 });
      } else {
        setTimeout(loadDeferredBackground, 0);
      }
    };

    if (!("PerformanceObserver" in window)) {
      window.addEventListener("load", trigger, { once: true });
      return;
    }

    try {
      const po = new PerformanceObserver(() => {});
      po.observe({ type: "largest-contentful-paint", buffered: true });

      const finalize = () => {
        po.disconnect();
        trigger();
      };

      window.addEventListener("pagehide", finalize, { once: true });
      window.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "hidden") finalize();
      });
      window.addEventListener("load", finalize, { once: true });
    } catch (e) {
      window.addEventListener("load", trigger, { once: true });
    }
  };

  deferBackgroundUntilAfterLCP();
})();
