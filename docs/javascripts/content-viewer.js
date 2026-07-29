(() => {
  const targetSelector = ".md-typeset img, .md-typeset .mermaid";
  let dialog;
  let stage;
  let scale = 1;
  let source;
  let observer;
  let placeholder;
  let movedTarget = false;
  let pageScrollY = 0;
  let returnFocus = false;

  const updateScale = () => {
    stage?.style.setProperty("--retsu-zoom-scale", scale);
  };

  const createButton = (label, text, action) => {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = text;
    button.title = label;
    button.setAttribute("aria-label", label);
    button.addEventListener("click", action);
    return button;
  };

  const ensureDialog = () => {
    if (dialog?.isConnected) {
      return;
    }

    dialog = document.createElement("dialog");
    dialog.className = "retsu-zoom";
    dialog.tabIndex = -1;
    dialog.setAttribute("aria-label", "Image and diagram viewer");

    const toolbar = document.createElement("div");
    toolbar.className = "retsu-zoom__toolbar";

    stage = document.createElement("div");
    stage.className = "retsu-zoom__stage";

    const viewport = document.createElement("div");
    viewport.className = "retsu-zoom__viewport";
    viewport.append(stage);

    toolbar.append(
      createButton("Zoom out", "−", () => {
        scale = Math.max(0.5, scale - 0.25);
        updateScale();
      }),
      createButton("Reset zoom", "1:1", () => {
        scale = 1;
        updateScale();
      }),
      createButton("Zoom in", "+", () => {
        scale = Math.min(3, scale + 0.25);
        updateScale();
      }),
      createButton("Close viewer", "×", () => dialog.close()),
    );

    dialog.append(toolbar, viewport);
    dialog.addEventListener("close", () => {
      if (movedTarget && placeholder?.isConnected) {
        placeholder.replaceWith(source);
      }

      stage.replaceChildren();
      document.documentElement.classList.remove("retsu-zoom-open");
      document.body.style.removeProperty("top");
      window.scrollTo(0, pageScrollY);
      movedTarget = false;
      placeholder = undefined;
      if (returnFocus) {
        source?.focus();
      }
      returnFocus = false;
    });
    document.body.append(dialog);
  };

  const openViewer = (target, focusControls) => {
    ensureDialog();
    if (dialog.open) {
      return;
    }

    movedTarget = target.matches(".mermaid");
    let content;

    if (movedTarget) {
      placeholder = document.createElement("div");
      placeholder.className = "retsu-zoom__placeholder";
      placeholder.style.height = `${target.getBoundingClientRect().height}px`;
      placeholder.setAttribute("aria-hidden", "true");
      target.before(placeholder);
      content = target;
    } else {
      content = target.cloneNode(true);
      content.removeAttribute("data-retsu-zoom-ready");
      content.removeAttribute("role");
      content.removeAttribute("tabindex");
      content.removeAttribute("aria-label");
    }

    source = target;
    scale = 1;
    pageScrollY = window.scrollY;
    returnFocus = focusControls;
    stage.replaceChildren(content);
    updateScale();
    document.documentElement.classList.add("retsu-zoom-open");
    document.body.style.top = `-${pageScrollY}px`;
    dialog.showModal();

    if (focusControls) {
      dialog.querySelector('button[aria-label="Zoom in"]').focus();
    } else {
      dialog.focus({ preventScroll: true });
    }
  };

  const prepareTargets = () => {
    document.querySelectorAll(targetSelector).forEach((target) => {
      if (target.dataset.retsuZoomReady) {
        return;
      }

      const label = target.matches("img")
        ? `Open ${target.alt || "image"} in viewer`
        : "Open diagram in viewer";

      target.dataset.retsuZoomReady = "true";
      target.setAttribute("role", "button");
      target.setAttribute("tabindex", "0");
      target.setAttribute("aria-label", label);

      target.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        openViewer(target, false);
      });
      target.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          openViewer(target, true);
        }
      });
    });
  };

  const initialize = () => {
    prepareTargets();
    observer?.disconnect();

    const content = document.querySelector(".md-content");
    if (content) {
      observer = new MutationObserver(prepareTargets);
      observer.observe(content, { childList: true, subtree: true });
    }
  };

  if (typeof document$ !== "undefined") {
    document$.subscribe(initialize);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initialize, { once: true });
  } else {
    initialize();
  }
})();
