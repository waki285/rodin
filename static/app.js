(() => {
  const sticky = document.getElementById("fixed-header");
  const primary = document.getElementById("primary-header");
  if (sticky && primary) {
    sticky.classList.remove("hidden");
    const update = (active) => {
      sticky.classList.toggle("opacity-100", active);
      sticky.classList.toggle("translate-y-0", active);
      sticky.classList.toggle("pointer-events-auto", active);
      sticky.classList.toggle("opacity-0", !active);
      sticky.classList.toggle("-translate-y-3", !active);
      sticky.classList.toggle("pointer-events-none", !active);
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
      btn.classList.remove("underline", "underline-offset-2");
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
})();
