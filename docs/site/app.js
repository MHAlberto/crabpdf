const root = document.documentElement;
const themeButton = document.querySelector("[data-theme-toggle]");
const systemTheme = matchMedia("(prefers-color-scheme: dark)");
const themeKey = "crabpdf-site-theme";
const explicitScheme = !CSS.supports("color", "light-dark(white, black)");

function showTheme() {
  const dark = root.dataset.theme === "dark" || (!root.dataset.theme && systemTheme.matches);
  const label = dark ? "Cambiar a tema claro" : "Cambiar a tema oscuro";
  themeButton.setAttribute("aria-label", label);
  themeButton.title = label;
  return dark;
}

if (themeButton) {
  try {
    const saved = localStorage.getItem(themeKey);
    if (saved === "dark" || saved === "light") root.dataset.theme = saved;
  } catch {
    // The system theme is still available when local storage is blocked.
  }
  if (explicitScheme && !root.dataset.theme) root.dataset.theme = systemTheme.matches ? "dark" : "light";
  showTheme();
  themeButton.hidden = false;
  themeButton.addEventListener("click", () => {
    root.dataset.theme = showTheme() ? "light" : "dark";
    showTheme();
    try {
      localStorage.setItem(themeKey, root.dataset.theme);
    } catch {
      // The selected theme still applies to this page.
    }
  });
  systemTheme.addEventListener("change", showTheme);
}

const download = document.querySelector("[data-download]");
if (download) {
  const platform = navigator.userAgentData?.platform || navigator.platform || navigator.userAgent;
  const mobile = /Android|iPhone|iPad|Mobile/i.test(navigator.userAgent)
    || (/Mac/i.test(platform) && navigator.maxTouchPoints > 1);
  if (!mobile) {
    const id = /^Win/i.test(platform) ? "windows"
      : /Mac/i.test(platform) ? "macos"
      : /Linux|X11/i.test(platform) ? "linux"
      : "";
    const section = id && document.getElementById(id);
    if (section?.dataset.downloadLabel) {
      download.href = "#" + id;
      download.textContent = section.dataset.downloadLabel;
    }
  }
}

for (const button of document.querySelectorAll("[data-copy]")) {
  const target = document.getElementById(button.dataset.copy);
  const status = document.getElementById(button.getAttribute("aria-describedby"));
  if (!target || !status) continue;
  button.hidden = false;
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(target.textContent.trim());
      status.textContent = button.dataset.copied;
    } catch {
      const range = document.createRange();
      range.selectNodeContents(target);
      const selection = getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      status.textContent = "Texto seleccionado. Puedes copiarlo con el teclado.";
    }
  });
}
