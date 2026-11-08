/** Top-level documentation navigation item. */
export type NavigationItem = {
  /** Link label. */
  label: string;
  /** Link destination. */
  href: string;
  /** Whether the destination is outside this site. */
  external?: boolean;
};

/** Sidebar navigation group. */
export type SidebarSection = {
  /** Group label. */
  title: string;
  /** Links rendered in the group. */
  items: NavigationItem[];
};

/** Page metadata consumed by the global site shell. */
export type DocsPageMetadata = {
  /** Browser page title. */
  title: string;
  /** Search and social description. */
  description: string;
};
