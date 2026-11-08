import type { NavigationItem, SidebarSection } from "../../entities/docs-page/model.js";

export const GITHUB_URL = "https://github.com/necoz-oss/type-tune";

export const primaryNavigation: NavigationItem[] = [
  { label: "Home", href: "/" },
  { label: "Usage", href: "/usage" },
  { label: "Config", href: "/config" },
];

export const sidebarSections: SidebarSection[] = [
  {
    title: "Guide",
    items: [
      { label: "Getting started", href: "/usage/getting-started" },
      { label: "Why type-tune", href: "/usage/why-type-tune" },
      { label: "Workflow", href: "/usage/workflow" },
      { label: "Usecases", href: "/usage/usecases" },
    ],
  },
  {
    title: "Config",
    items: [
      { label: "Shared options", href: "/config/shared-options" },
      { label: "Scan options", href: "/config/scan-options" },
      { label: "Plan options", href: "/config/plan-options" },
      { label: "Review options", href: "/config/review-options" },
      { label: "Apply options", href: "/config/apply-options" },
      { label: "Extract options", href: "/config/extract-options" },
      { label: "AI review options", href: "/config/ai-review-options" },
    ],
  },
];

export const NPM_PACKAGE_URL = "https://www.npmjs.com/package/type-tune";
export const RELEASES_URL = "https://github.com/necoz-oss/type-tune/releases";

export const resourceNavigation: NavigationItem[] = [
  { label: "npm", href: NPM_PACKAGE_URL, external: true },
  { label: "Releases", href: RELEASES_URL, external: true },
];
