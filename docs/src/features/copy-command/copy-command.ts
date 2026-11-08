import { isInstanceOf } from "is-kit";

const isHtmlButtonElement = isInstanceOf(HTMLButtonElement);

/** Initializes copy buttons within a DOM subtree. */
export function setupCopyCommands(root: ParentNode = document): void {
  const copyButtons = [...root.querySelectorAll("[data-copy-command]")].filter(isHtmlButtonElement);

  for (const button of copyButtons) {
    if (button.dataset.copyReady === "true") continue;
    button.dataset.copyReady = "true";
    button.addEventListener("click", async () => {
      const targetId = button.dataset.copyCommand;
      const target = targetId ? document.getElementById(targetId) : null;
      const command = target?.textContent ?? "";
      if (!command) return;

      await navigator.clipboard.writeText(command);
      const originalLabel = button.textContent;
      button.textContent = "Copied";
      window.setTimeout(() => {
        button.textContent = originalLabel;
      }, 1400);
    });
  }
}
