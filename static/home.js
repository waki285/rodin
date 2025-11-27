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
})();
