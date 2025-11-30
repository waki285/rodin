(() => {
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
    btn.addEventListener("click", () => {
      const current = document.documentElement.classList.contains("dark")
        ? "dark"
        : "light";
      const next = current === "dark" ? "light" : "dark";
      applyTheme(next);
      localStorage.setItem(THEME_KEY, next);
    });
  });

  document.querySelectorAll("[data-show-ip]").forEach((btn) => {
    if (!(btn instanceof HTMLElement)) return;
    btn.addEventListener("click", () => {
      const ip = btn.getAttribute("data-show-ip");
      if (!ip) return;
      btn.textContent = "IP: " + ip;
      btn.classList.remove("underline");
    });
  });

  // Close nav menus when clicking outside or selecting a link (mobile)
  const navToggles = [
    document.getElementById("nav-toggle-main"),
    document.getElementById("nav-toggle-fixed"),
  ].filter((x) => x);

  const closeMenus = () => navToggles.forEach((chk) => (chk.checked = false));

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

  // Prefetch in-page links (similar to Next.js) after idle so LCP is unaffected
  const schedulePrefetch = () => {
    const addPrefetch = (href) => {
      if (!href || href.startsWith("http")) return;
      // avoid duplicate prefetch tags
      if (document.querySelector(`link[rel="prefetch"][href="${href}"]`)) return;
      const l = document.createElement("link");
      l.rel = "prefetch";
      l.as = "document";
      l.href = href;
      document.head.appendChild(l);
    };

    const links = document.querySelectorAll("a[data-prefetch='true']");
    links.forEach((a) => addPrefetch(a.getAttribute("href")));
  };

  if ("requestIdleCallback" in window) {
    requestIdleCallback(schedulePrefetch, { timeout: 3000 });
  } else {
    setTimeout(schedulePrefetch, 2000);
  }

  // Add copy buttons to code blocks
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
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initCopyButtons);
  } else {
    initCopyButtons();
  }

  // Lazy-load home-only script after parse to shrink the critical path.
  if (window.location.pathname === "/") {
    const loadHomeJs = () => import("/assets/build/home.js").catch(() => {});
    const scheduleHome = () => {
      if ("requestIdleCallback" in window) {
        requestIdleCallback(loadHomeJs, { timeout: 1500 });
      } else {
        setTimeout(loadHomeJs, 0);
      }
    };

    if (document.readyState === "complete") {
      scheduleHome();
    } else {
      window.addEventListener("load", scheduleHome, { once: true });
    }
  }
})();
