/** Supported package manager. */
export type PackageManager = "pnpm" | "yarn" | "bun" | "npm";

/** Package-manager installation command. */
export type InstallCommand = {
  /** Package manager label. */
  manager: PackageManager;
  /** Shell command to install type-tune. */
  command: string;
};

export const installCommands: InstallCommand[] = [
  { manager: "pnpm", command: "pnpm add -D type-tune" },
  { manager: "yarn", command: "yarn add -D type-tune" },
  { manager: "bun", command: "bun add -d type-tune" },
  { manager: "npm", command: "npm install -D type-tune" },
];
