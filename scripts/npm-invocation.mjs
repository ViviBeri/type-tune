import fs from "node:fs";
import path from "node:path";

/** Invoke npm without a shell, including when Windows exposes it only as npm.cmd. */
export function npmInvocation({ platform = process.platform, node = process.execPath, env = process.env, exists = fs.existsSync } = {}) {
  if (platform !== "win32") return { command: "npm", args: [] };
  const directories = [path.win32.dirname(node), ...(env.PATH ?? env.Path ?? "").split(";")];
  for (const directory of directories.filter(Boolean)) {
    const cli = path.win32.join(directory, "node_modules", "npm", "bin", "npm-cli.js");
    if (exists(cli)) return { command: node, args: [cli] };
  }
  throw new Error("Could not find npm-cli.js beside Node or in PATH; install npm with Node.js.");
}
