// Static enhancement only: downloads and all essential content also work without JS.
(() => {
  const root = document.documentElement;
  root.classList.add("js");
  try {
    const saved = localStorage.getItem("crabpdf-site-theme");
    if (saved === "dark" || saved === "light") root.dataset.theme = saved;
  } catch { /* Private browsing or blocked storage keeps the default theme. */ }

  document.addEventListener("DOMContentLoaded", () => {
    const themeButton = document.querySelector("#theme-toggle");
    const updateThemeLabel = () => {
      const dark = root.dataset.theme !== "light";
      themeButton.setAttribute("aria-label", dark ? "Cambiar a tema claro" : "Cambiar a tema oscuro");
      themeButton.setAttribute("aria-pressed", String(!dark));
    };
    updateThemeLabel();
    themeButton.addEventListener("click", () => {
      root.dataset.theme = root.dataset.theme === "light" ? "dark" : "light";
      try { localStorage.setItem("crabpdf-site-theme", root.dataset.theme); } catch {}
      updateThemeLabel();
    });

    const menuButton = document.querySelector("#nav-toggle");
    const nav = document.querySelector("#main-nav");
    const setMenu = (open) => {
      menuButton.setAttribute("aria-expanded", String(open));
      menuButton.setAttribute("aria-label", open ? "Cerrar menú" : "Abrir menú");
      nav.classList.toggle("is-open", open);
    };
    menuButton.addEventListener("click", () => setMenu(menuButton.getAttribute("aria-expanded") !== "true"));
    nav.addEventListener("click", (event) => { if (event.target.closest("a")) setMenu(false); });
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && menuButton.getAttribute("aria-expanded") === "true") {
        setMenu(false); menuButton.focus();
      }
    });

    // Tab behavior follows the WAI-ARIA pattern. Without JS, every panel stays visible.
    for (const tablist of document.querySelectorAll('[role="tablist"]')) {
      const tabs = [...tablist.querySelectorAll('[role="tab"]')];
      const activate = (current, focus = false) => {
        for (const tab of tabs) {
          const selected = tab === current;
          tab.setAttribute("aria-selected", String(selected));
          tab.tabIndex = selected ? 0 : -1;
          document.getElementById(tab.getAttribute("aria-controls")).hidden = !selected;
        }
        if (focus) current.focus();
      };
      const initial = tabs.find((tab) => tab.getAttribute("aria-selected") === "true") || tabs[0];
      activate(initial);
      tablist.addEventListener("click", (event) => {
        const tab = event.target.closest('[role="tab"]');
        if (tabs.includes(tab)) activate(tab);
      });
      tablist.addEventListener("keydown", (event) => {
        if (event.altKey || event.ctrlKey || event.metaKey) return;
        const index = tabs.indexOf(document.activeElement);
        if (index < 0) return;
        let next;
        if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
        if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
        if (event.key === "Home") next = 0;
        if (event.key === "End") next = tabs.length - 1;
        if (next !== undefined) { event.preventDefault(); activate(tabs[next], true); }
      });
    }

    const copyButton = document.querySelector("#copy-install");
    copyButton.addEventListener("click", async () => {
      const command = document.querySelector("#ubuntu-command").textContent;
      const status = document.querySelector("#copy-status");
      try {
        await navigator.clipboard.writeText(command);
        status.textContent = "Comando copiado.";
      } catch {
        status.textContent = "Selecciona y copia el comando que aparece arriba.";
      }
    });
  });
})();