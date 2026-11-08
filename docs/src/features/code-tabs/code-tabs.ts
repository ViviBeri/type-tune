import { isInstanceOf, isNil } from "is-kit";

const COPY_FEEDBACK_DURATION_MS = 1_400;

const isHtmlButtonElement = isInstanceOf(HTMLButtonElement);
const isHtmlCanvasElement = isInstanceOf(HTMLCanvasElement);
const isHtmlElement = isInstanceOf(HTMLElement);

const WAVE_CONFIG = {
  primaryFrequency: 0.036,
  secondaryFrequency: 0.081,
  primaryAmplitudePx: 1.15,
  secondaryAmplitudePx: 0.55,
  secondaryPhaseScale: -0.7,
  staticPhase: 0.8,
} as const;

const DRAW_CONFIG = {
  minimumBitmapSizePx: 1,
  fallbackPixelRatio: 1,
  lineThicknessPx: 2,
  outlineInsetPx: 4,
  dividerHorizontalInsetPx: 8,
  dividerYOffsetPx: 1,
} as const;

const PHASE_OFFSETS = {
  rightEdge: 1.7,
  bottomEdge: 2.8,
  leftEdge: 4.1,
  divider: 0.9,
} as const;

const AMPLITUDE_SCALES = {
  verticalEdge: 0.65,
  divider: 0.7,
} as const;

type WavyOutlineElements = {
  tabs: HTMLElement;
  canvas: HTMLCanvasElement;
  tabList: HTMLElement;
  context: CanvasRenderingContext2D;
};

type PixelLineOptions = {
  start: number;
  end: number;
  baseline: number;
  phase: number;
  amplitudeScale?: number;
  pixelRatio: number;
};

/** Initializes package-manager tabs, copying, and hand-drawn canvas outlines. */
export function setupCodeTabs(): void {
  const tabContainers = [...document.querySelectorAll("[data-code-tabs]")].filter(isHtmlElement);

  for (const tabs of tabContainers) {
    if (tabs.dataset.ready === "true") {
      continue;
    }

    tabs.dataset.ready = "true";
    setupTabSelection(tabs);
    setupCopyButtons(tabs);
    setupWavyOutline(tabs);
  }
}

function setupTabSelection(tabs: HTMLElement): void {
  const buttons = [...tabs.querySelectorAll("[role='tab']")].filter(isHtmlButtonElement);
  const panels = [...tabs.querySelectorAll("[role='tabpanel']")].filter(isHtmlElement);

  buttons.forEach((button, selectedIndex) => {
    button.addEventListener("keydown", (event) => {
      let nextIndex: number;
      switch (event.key) {
        case "ArrowRight":
          nextIndex = (selectedIndex + 1) % buttons.length;
          break;
        case "ArrowLeft":
          nextIndex = (selectedIndex - 1 + buttons.length) % buttons.length;
          break;
        case "Home":
          nextIndex = 0;
          break;
        case "End":
          nextIndex = buttons.length - 1;
          break;
        default:
          return;
      }
      event.preventDefault();
      buttons[nextIndex]?.click();
      buttons[nextIndex]?.focus();
    });
    button.addEventListener("click", () => {
      buttons.forEach((item, itemIndex) => {
        const isSelected = itemIndex === selectedIndex;
        item.setAttribute("aria-selected", String(isSelected));
        item.tabIndex = isSelected ? 0 : -1;
      });
      panels.forEach((panel, panelIndex) => {
        panel.hidden = panelIndex !== selectedIndex;
      });
    });
  });
}

function setupCopyButtons(tabs: HTMLElement): void {
  const copyButtons = [...tabs.querySelectorAll("[data-copy-value]")].filter(isHtmlButtonElement);

  for (const button of copyButtons) {
    button.addEventListener("click", () => void copyButtonValue(button));
  }
}

async function copyButtonValue(button: HTMLButtonElement): Promise<void> {
  await navigator.clipboard.writeText(button.dataset.copyValue ?? "");
  const label = button.querySelector<HTMLElement>("[data-copy-label]") ?? button;
  label.textContent = "Copied";
  window.setTimeout(() => (label.textContent = "Copy"), COPY_FEEDBACK_DURATION_MS);
}

function setupWavyOutline(tabs: HTMLElement): void {
  const elements = getWavyOutlineElements(tabs);
  if (isNil(elements)) {
    return;
  }

  const draw = () => drawWavyOutline(elements, WAVE_CONFIG.staticPhase);
  new ResizeObserver(draw).observe(tabs);
  draw();
}

function getWavyOutlineElements(tabs: HTMLElement): WavyOutlineElements | undefined {
  const canvas = tabs.querySelector("canvas");
  const tabList = tabs.querySelector("[data-code-tabs-heading]");

  if (!isHtmlCanvasElement(canvas) || !isHtmlElement(tabList)) {
    return undefined;
  }

  const context = canvas.getContext("2d");

  if (isNil(context)) {
    return undefined;
  }

  return { tabs, canvas, tabList, context };
}

function drawWavyOutline(elements: WavyOutlineElements, phase: number): void {
  const { tabs, canvas, tabList, context } = elements;
  const { width, height, left, top } = tabs.getBoundingClientRect();
  const pixelRatio = window.devicePixelRatio || DRAW_CONFIG.fallbackPixelRatio;

  resizeCanvasBitmap(canvas, width, height, pixelRatio);
  alignCanvasToDevicePixels(canvas, left, top, pixelRatio);
  prepareDrawingContext(context, tabs);
  drawOuterPath(context, width, height, phase, pixelRatio);
  drawDividerPath(context, width, tabList.offsetHeight, phase, pixelRatio);
}

function resizeCanvasBitmap(
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
  pixelRatio: number,
): void {
  const bitmapWidth = Math.max(DRAW_CONFIG.minimumBitmapSizePx, Math.round(width * pixelRatio));
  const bitmapHeight = Math.max(DRAW_CONFIG.minimumBitmapSizePx, Math.round(height * pixelRatio));

  if (canvas.width !== bitmapWidth || canvas.height !== bitmapHeight) {
    canvas.width = bitmapWidth;
    canvas.height = bitmapHeight;
  }

  canvas.style.width = `${bitmapWidth / pixelRatio}px`;
  canvas.style.height = `${bitmapHeight / pixelRatio}px`;
}

function alignCanvasToDevicePixels(
  canvas: HTMLCanvasElement,
  left: number,
  top: number,
  pixelRatio: number,
): void {
  const offsetX = Math.round(left * pixelRatio) / pixelRatio - left;
  const offsetY = Math.round(top * pixelRatio) / pixelRatio - top;

  canvas.style.left = `${offsetX}px`;
  canvas.style.top = `${offsetY}px`;
}

function prepareDrawingContext(context: CanvasRenderingContext2D, tabs: HTMLElement): void {
  context.setTransform(1, 0, 0, 1, 0, 0);
  context.clearRect(0, 0, context.canvas.width, context.canvas.height);
  context.fillStyle = getComputedStyle(tabs).getPropertyValue("--secondary").trim();
}

function drawOuterPath(
  context: CanvasRenderingContext2D,
  width: number,
  height: number,
  phase: number,
  pixelRatio: number,
): void {
  const inset = DRAW_CONFIG.outlineInsetPx;

  drawHorizontalPixelLine(context, {
    start: inset,
    end: width - inset,
    baseline: inset,
    phase,
    pixelRatio,
  });
  drawVerticalPixelLine(context, {
    start: inset,
    end: height - inset,
    baseline: width - inset,
    phase: phase + PHASE_OFFSETS.rightEdge,
    amplitudeScale: AMPLITUDE_SCALES.verticalEdge,
    pixelRatio,
  });
  drawHorizontalPixelLine(context, {
    start: inset,
    end: width - inset,
    baseline: height - inset,
    phase: phase + PHASE_OFFSETS.bottomEdge,
    pixelRatio,
  });
  drawVerticalPixelLine(context, {
    start: inset,
    end: height - inset,
    baseline: inset,
    phase: phase + PHASE_OFFSETS.leftEdge,
    amplitudeScale: AMPLITUDE_SCALES.verticalEdge,
    pixelRatio,
  });
}

function drawDividerPath(
  context: CanvasRenderingContext2D,
  width: number,
  tabListHeight: number,
  phase: number,
  pixelRatio: number,
): void {
  const inset = DRAW_CONFIG.outlineInsetPx;
  const startX = inset + DRAW_CONFIG.dividerHorizontalInsetPx;
  const endX = width - inset - DRAW_CONFIG.dividerHorizontalInsetPx;
  const dividerY = tabListHeight + DRAW_CONFIG.dividerYOffsetPx;

  drawHorizontalPixelLine(context, {
    start: startX,
    end: endX,
    baseline: dividerY,
    phase: phase + PHASE_OFFSETS.divider,
    amplitudeScale: AMPLITUDE_SCALES.divider,
    pixelRatio,
  });
}

function drawHorizontalPixelLine(
  context: CanvasRenderingContext2D,
  options: PixelLineOptions,
): void {
  const { start, end, baseline, phase, amplitudeScale = 1, pixelRatio } = options;
  const startPixel = Math.round(start * pixelRatio);
  const endPixel = Math.round(end * pixelRatio);
  const thickness = getDeviceLineThickness(pixelRatio);

  for (let x = startPixel; x <= endPixel; x += 1) {
    const position = x / pixelRatio;
    const centerY = Math.round(
      (baseline + getWaveOffset(position, phase, amplitudeScale)) * pixelRatio,
    );
    const top = centerY - Math.floor(thickness / 2);

    context.fillRect(x, top, 1, thickness);
  }
}

function drawVerticalPixelLine(context: CanvasRenderingContext2D, options: PixelLineOptions): void {
  const { start, end, baseline, phase, amplitudeScale = 1, pixelRatio } = options;
  const startPixel = Math.round(start * pixelRatio);
  const endPixel = Math.round(end * pixelRatio);
  const thickness = getDeviceLineThickness(pixelRatio);

  for (let y = startPixel; y <= endPixel; y += 1) {
    const position = y / pixelRatio;
    const centerX = Math.round(
      (baseline + getWaveOffset(position, phase, amplitudeScale)) * pixelRatio,
    );
    const left = centerX - Math.floor(thickness / 2);

    context.fillRect(left, y, thickness, 1);
  }
}

function getDeviceLineThickness(pixelRatio: number): number {
  return Math.max(
    DRAW_CONFIG.minimumBitmapSizePx,
    Math.round(DRAW_CONFIG.lineThicknessPx * pixelRatio),
  );
}

function getWaveOffset(position: number, phase: number, amplitudeScale = 1): number {
  const primaryWave =
    Math.sin(position * WAVE_CONFIG.primaryFrequency + phase) * WAVE_CONFIG.primaryAmplitudePx;
  const secondaryWave =
    Math.sin(position * WAVE_CONFIG.secondaryFrequency + phase * WAVE_CONFIG.secondaryPhaseScale) *
    WAVE_CONFIG.secondaryAmplitudePx;

  return (primaryWave + secondaryWave) * amplitudeScale;
}
