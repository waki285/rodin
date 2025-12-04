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

  // Delay swapping to high-quality hero until after LCP is settled.
  const loadDeferredBackground = async () => {
    const picture = document.querySelector("[data-deferred-bg]");
    if (!picture || picture.dataset.loaded === "1") return;

    const img = picture.querySelector("img[data-src]");
    if (!(img instanceof HTMLImageElement)) return;

    // Collect the high-quality URLs to preload
    const avifHi = picture.querySelector('source[type="image/avif"][data-hi-srcset]');
    const hiSrcset = avifHi?.getAttribute("data-hi-srcset");

    // Extract the actual URL from srcset (e.g., "/path/image.avif 2560w" -> "/path/image.avif")
    const extractUrl = (srcset) => {
      if (!srcset) return null;
      const match = srcset.split(",").map(s => s.trim().split(/\s+/)[0]).filter(Boolean);
      return match.length > 0 ? match[match.length - 1] : null; // Get the largest one
    };

    const preloadUrl = extractUrl(hiSrcset);

    // Preload the high-quality image using fetch to warm the cache
    if (preloadUrl) {
      try {
        const response = await fetch(preloadUrl);
        // Create a blob and decode it
        const blob = await response.blob();
        const bitmapUrl = URL.createObjectURL(blob);
        const preloadImg = new Image();
        preloadImg.src = bitmapUrl;
        await preloadImg.decode();
        URL.revokeObjectURL(bitmapUrl);
      } catch {
        // Preload failed, continue anyway
      }
    }

    // Now swap sources - the image should be in browser cache
    if (avifHi instanceof HTMLSourceElement && hiSrcset) {
      avifHi.srcset = hiSrcset;
      avifHi.removeAttribute("data-hi-srcset");
    }

    picture.querySelectorAll("source[data-srcset]").forEach((source) => {
      const set = source.getAttribute("data-srcset");
      if (set) {
        source.srcset = set;
        source.removeAttribute("data-srcset");
      }
    });

    const sizes = img.getAttribute("data-sizes") || picture.getAttribute("data-bg-sizes");
    if (sizes) img.sizes = sizes;
    const srcset = img.getAttribute("data-srcset");
    if (srcset) img.srcset = srcset;
    const src = img.getAttribute("data-src");
    if (src) img.src = src;
    img.fetchPriority = "high";
    img.loading = "eager";

    // Wait for the actual img element to decode
    try {
      await img.decode();
    } catch {
      // Decode failed
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
