(() => {
  // ============================================
  // Trusted Types Policies
  // ============================================
  const getOrCreatePolicy = (name, spec) => {
    const tt = window.trustedTypes;
    if (!tt) return null;
    const existing = tt.getPolicy?.(name);
    if (existing) return existing;
    try {
      return tt.createPolicy(name, spec);
    } catch (_) {
      return tt.getPolicy?.(name) ?? null;
    }
  };

  // Default policy to satisfy require-trusted-types-for 'script'
  getOrCreatePolicy("default", {
    createHTML: () => { throw new Error("Not allowed"); },
    createScriptURL: (url) => {
      if (url.startsWith("https://platform.twitter.com/js/tweet.")) return url;
      throw new Error("Not allowed");
    },
  });

  const trustedPolicy = getOrCreatePolicy("rodin-spa", {
    createHTML: (html) => html,
  });

  const safeHTML = (html) => (trustedPolicy ? trustedPolicy.createHTML(html) : html);

  // ============================================
  // Client-side Router (Next.js/Qwik style)
  // ============================================
  const Router = (() => {
    const cache = new Map(); // URL -> { html, title, timestamp }
    const CACHE_TTL = 5 * 60 * 1000; // 5 minutes
    const prefetching = new Set();
    let isNavigating = false;

    // Check if URL is internal and should be handled by router
    const isInternalLink = (url) => {
      try {
        const parsed = new URL(url, location.origin);
        if (parsed.origin !== location.origin) return false;
        if (parsed.pathname.match(/\.(pdf|zip|tar|gz|xml|txt|typ|md|json)$/i)) return false;
        if (parsed.pathname.startsWith("/assets/")) return false;
        if (parsed.pathname.startsWith("/__admin")) return false;
        return true;
      } catch {
        return false;
      }
    };

    // Fetch and parse HTML
    const fetchPage = async (url) => {
      const cached = cache.get(url);
      if (cached && Date.now() - cached.timestamp < CACHE_TTL) {
        return cached;
      }

      const response = await fetch(url, {
        headers: { "X-Rodin-SPA": "1" },
        credentials: "same-origin",
      });

      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const html = await response.text();
      const parser = new DOMParser();
      // DOMParser also requires TrustedHTML in strict CSP environments
      const doc = parser.parseFromString(safeHTML(html), "text/html");
      const title = doc.title || "すずねーう";
      const bodyHtml = doc.body.innerHTML;
      
      // Extract stylesheet links from the new page
      const stylesheets = Array.from(doc.querySelectorAll('link[rel="stylesheet"]'))
        .map((l) => l.getAttribute("href"))
        .filter(Boolean);

      // Extract script tags from the new page (excluding inline scripts and app.js)
      const scripts = Array.from(doc.querySelectorAll('script[src]'))
        .map((s) => s.getAttribute("src"))
        .filter((src) => src && !src.includes("app.js") && !src.includes("app-"));

      const entry = { html: bodyHtml, title, stylesheets, scripts, timestamp: Date.now() };
      cache.set(url, entry);
      return entry;
    };

    // Prefetch a URL (low priority)
    const prefetch = async (url) => {
      if (cache.has(url) || prefetching.has(url)) return;
      prefetching.add(url);
      try {
        await fetchPage(url);
      } catch {
        // Ignore prefetch errors
      } finally {
        prefetching.delete(url);
      }
    };

    // Ensure all required stylesheets are loaded
    const ensureStylesheets = (stylesheets) => {
      const currentHrefs = new Set(
        Array.from(document.querySelectorAll('link[rel="stylesheet"]'))
          .map((l) => l.getAttribute("href"))
          .filter(Boolean)
      );

      const promises = [];
      for (const href of stylesheets) {
        if (!currentHrefs.has(href)) {
          // Add missing stylesheet
          const link = document.createElement("link");
          link.rel = "stylesheet";
          link.href = href;
          
          // Wait for it to load
          const loadPromise = new Promise((resolve) => {
            link.onload = resolve;
            link.onerror = resolve; // Don't block on error
          });
          promises.push(loadPromise);
          
          document.head.appendChild(link);
        }
      }
      return Promise.all(promises);
    };

    // Ensure all required scripts are loaded
    const ensureScripts = (scripts) => {
      const currentSrcs = new Set(
        Array.from(document.querySelectorAll('script[src]'))
          .map((s) => s.getAttribute("src"))
          .filter(Boolean)
      );

      const promises = [];
      for (const src of scripts) {
        if (!currentSrcs.has(src)) {
          // Add missing script
          const script = document.createElement("script");
          script.src = src;
          script.async = true;
          
          // Wait for it to load
          const loadPromise = new Promise((resolve) => {
            script.onload = resolve;
            script.onerror = resolve; // Don't block on error
          });
          promises.push(loadPromise);
          
          document.body.appendChild(script);
        }
      }
      return Promise.all(promises);
    };

    // Update DOM with new page content
    const updateDOM = async (entry, url) => {
      // Ensure all stylesheets are loaded first
      await ensureStylesheets(entry.stylesheets);

      // Update title
      document.title = entry.title;

      // Swap body content
      document.body.innerHTML = safeHTML(entry.html);

      // Load page-specific scripts after DOM update
      await ensureScripts(entry.scripts || []);

      // Re-run initialization scripts
      reinitialize();

      // Scroll to top (or hash)
      const hash = new URL(url, location.origin).hash;
      if (hash) {
        const target = document.querySelector(hash);
        if (target) {
          target.scrollIntoView({ behavior: "instant" });
          return;
        }
      }
      window.scrollTo(0, 0);
    };

    // Navigate to a new URL
    // pushState: true = normal navigation, false = popstate (back/forward)
    const navigate = async (url, pushState = true) => {
      if (isNavigating) return;
      // Only skip duplicate for normal navigation, not for popstate
      if (pushState && url === location.href) return;

      isNavigating = true;

      try {
        const entry = await fetchPage(url);

        // Use View Transitions API if available
        if (document.startViewTransition) {
          await document.startViewTransition(async () => {
            await updateDOM(entry, url);
          }).finished;
        } else {
          await updateDOM(entry, url);
        }

        if (pushState) {
          history.pushState({ url }, "", url);
        }
      } catch (err) {
        // Fallback to normal navigation on error
        location.href = url;
      } finally {
        isNavigating = false;
      }
    };

    // Handle click events on links
    const handleClick = (e) => {
      // Ignore if modifier keys pressed
      if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      if (e.button !== 0) return; // Left click only

      const link = e.target.closest("a");
      if (!link) return;

      const href = link.getAttribute("href");
      if (!href) return;

      const url = new URL(href, location.origin).href;
      if (!isInternalLink(url)) return;

      // Check for download or target="_blank"
      if (link.hasAttribute("download")) return;
      if (link.target === "_blank") return;

      e.preventDefault();
      navigate(url);
    };

    // Handle popstate (back/forward)
    const handlePopState = (e) => {
      const url = e.state?.url || location.href;
      navigate(url, false);
    };

    // Prefetch on hover/focus
    const handleHover = (e) => {
      const link = e.target.closest("a");
      if (!link) return;

      const href = link.getAttribute("href");
      if (!href) return;

      const url = new URL(href, location.origin).href;
      if (!isInternalLink(url)) return;

      prefetch(url);
    };

    // Initialize router
    const init = () => {
      // Store initial state
      history.replaceState({ url: location.href }, "", location.href);

      // Event listeners
      document.addEventListener("click", handleClick);
      window.addEventListener("popstate", handlePopState);

      // Prefetch on hover (with debounce)
      let hoverTimeout;
      document.addEventListener("mouseover", (e) => {
        clearTimeout(hoverTimeout);
        hoverTimeout = setTimeout(() => handleHover(e), 50);
      });
      document.addEventListener("focusin", handleHover);

      // Prefetch visible links in viewport after idle
      if ("IntersectionObserver" in window) {
        const prefetchObserver = new IntersectionObserver(
          (entries) => {
            entries.forEach((entry) => {
              if (entry.isIntersecting) {
                const href = entry.target.getAttribute("href");
                if (href) {
                  const url = new URL(href, location.origin).href;
                  if (isInternalLink(url)) {
                    prefetch(url);
                  }
                }
                prefetchObserver.unobserve(entry.target);
              }
            });
          },
          { rootMargin: "50px" }
        );

        const observeLinks = () => {
          document.querySelectorAll("a[data-prefetch='true']").forEach((link) => {
            prefetchObserver.observe(link);
          });
        };

        if ("requestIdleCallback" in window) {
          requestIdleCallback(observeLinks, { timeout: 3000 });
        } else {
          setTimeout(observeLinks, 2000);
        }
      }
    };

    return { init, prefetch, navigate };
  })();

  // ============================================
  // Reinitialize scripts after SPA navigation
  // ============================================
  const reinitialize = () => {
    initStickyHeader();
    initThemeToggle();
    initShowIp();
    initNavMenus();
    initCopyButtons();
    initHomeScript();
  };

  // ============================================
  // Sticky Header
  // ============================================
  const initStickyHeader = () => {
    const sticky = document.getElementById("fixed-header");
    const primary = document.getElementById("primary-header");
    if (sticky && primary) {
      sticky.classList.remove("hidden");
      const update = (active) => {
        sticky.classList.toggle("active", active);
      };

      if ("IntersectionObserver" in window) {
        const observer = new IntersectionObserver(
          (entries) => {
            entries.forEach((entry) => {
              update(!entry.isIntersecting);
            });
          },
          { rootMargin: "0px", threshold: 0 }
        );
        observer.observe(primary);
      } else {
        const threshold = primary.offsetHeight;
        const onScroll = () => update(window.scrollY > threshold);
        window.addEventListener("scroll", onScroll, { passive: true });
        onScroll();
      }
    }
  };

  // ============================================
  // Theme Toggle
  // ============================================
  const THEME_KEY = "rodin-theme";
  
  const toggles = () =>
    Array.from(document.querySelectorAll(".theme-toggle")).filter(
      (btn) => btn instanceof HTMLElement
    );

  const applyTheme = (theme) => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    toggles().forEach((btn) => {
      btn.dataset.theme = theme;
      btn.setAttribute("aria-pressed", theme === "dark" ? "true" : "false");
    });
  };

  const initThemeToggle = () => {
    const stored = localStorage.getItem(THEME_KEY);
    const preferred =
      window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    const initial = stored || preferred;
    applyTheme(initial);
    localStorage.setItem(THEME_KEY, initial);

    toggles().forEach((btn) => {
      // Remove old listeners by cloning
      const newBtn = btn.cloneNode(true);
      btn.parentNode.replaceChild(newBtn, btn);
      newBtn.addEventListener("click", () => {
        const current = document.documentElement.classList.contains("dark")
          ? "dark"
          : "light";
        const next = current === "dark" ? "light" : "dark";
        applyTheme(next);
        localStorage.setItem(THEME_KEY, next);
      });
    });
  };

  // ============================================
  // Show IP Button
  // ============================================
  const initShowIp = () => {
    document.querySelectorAll("[data-show-ip]").forEach((btn) => {
      if (!(btn instanceof HTMLElement)) return;
      if (btn.dataset.ipInit) return;
      btn.dataset.ipInit = "1";
      btn.addEventListener("click", () => {
        const ip = btn.getAttribute("data-show-ip");
        if (!ip) return;
        btn.textContent = "IP: " + ip;
        btn.classList.remove("underline");
      });
    });
  };

  // ============================================
  // Navigation Menus
  // ============================================
  const initNavMenus = () => {
    const navToggles = [
      document.getElementById("nav-toggle-main"),
      document.getElementById("nav-toggle-fixed"),
    ].filter((x) => x);

    const closeMenus = () => navToggles.forEach((chk) => (chk.checked = false));

    // Close menus on outside click
    document.addEventListener("click", (e) => {
      const target = e.target;
      if (!(target instanceof Element)) return;
      const menuContainers = [
        document.getElementById("primary-header"),
        document.getElementById("fixed-header"),
      ].filter((x) => x);
      const inside = menuContainers.some((c) => c && c.contains(target));
      if (!inside) closeMenus();
    });

    document.querySelectorAll("#primary-header a, #fixed-header a").forEach((a) => {
      a.addEventListener("click", () => closeMenus());
    });
  };

  // ============================================
  // Copy Buttons for Code Blocks
  // ============================================
  const initCopyButtons = () => {
    document.querySelectorAll("pre code").forEach((code) => {
      const pre = code.parentElement;
      if (!(pre instanceof HTMLElement)) return;
      if (pre.dataset.copyInit) return;
      pre.dataset.copyInit = "1";
      pre.style.position = pre.style.position || "relative";
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "code-copy-btn";
      btn.textContent = "Copy";
      btn.ariaLabel = "コードをコピー";
      btn.addEventListener("click", () => {
        navigator.clipboard
          .writeText(code.innerText)
          .then(() => {
            btn.textContent = "Copied!";
            setTimeout(() => (btn.textContent = "Copy"), 1200);
          })
          .catch(() => {
            btn.textContent = "Failed";
            setTimeout(() => (btn.textContent = "Copy"), 1200);
          });
      });
      pre.appendChild(btn);
    });
  };

  // ============================================
  // Home-specific Script
  // ============================================
  const initHomeScript = () => {
    if (window.location.pathname !== "/") return;

    const loadHomeJs = () => import("/assets/build/home.js").catch(() => {});

    // Wait for LCP, then load home.js
    if ("PerformanceObserver" in window) {
      let lcpFired = false;
      const observer = new PerformanceObserver((list) => {
        const entries = list.getEntries();
        if (entries.length > 0 && !lcpFired) {
          lcpFired = true;
          observer.disconnect();
          requestAnimationFrame(() => {
            setTimeout(loadHomeJs, 0);
          });
        }
      });
      observer.observe({ type: "largest-contentful-paint", buffered: true });

      // Fallback: if LCP doesn't fire within 5s, load anyway
      setTimeout(() => {
        if (!lcpFired) {
          lcpFired = true;
          observer.disconnect();
          loadHomeJs();
        }
      }, 5000);
    } else {
      if (document.readyState === "complete") {
        "requestIdleCallback" in window
          ? requestIdleCallback(loadHomeJs, { timeout: 1500 })
          : setTimeout(loadHomeJs, 0);
      } else {
        window.addEventListener(
          "load",
          () => {
            "requestIdleCallback" in window
              ? requestIdleCallback(loadHomeJs, { timeout: 1500 })
              : setTimeout(loadHomeJs, 0);
          },
          { once: true }
        );
      }
    }
  };

  // ============================================
  // Initialize Everything
  // ============================================
  Router.init();
  initStickyHeader();
  initThemeToggle();
  initShowIp();
  initNavMenus();
  initCopyButtons();
  initHomeScript();
})();
