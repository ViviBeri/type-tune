import { execFileSync } from "node:child_process";

const minimum = "11.5.1";

function numericParts(version) {
  const match = String(version).trim().match(/^(\d+)\.(\d+)\.(\d+)$/);
  return match ? match.slice(1, 4).map(Number) : undefined;
}

function isAtLeast(version, requiredVersion) {
  const actual = numericParts(version);
  const required = numericParts(requiredVersion);
  if (!actual || !required) return false;
  for (let index = 0; index < required.length; index += 1) {
    if (actual[index] > required[index]) return true;
    if (actual[index] < required[index]) return false;
  }
  return true;
}

const version = process.argv[2] ?? execFileSync("npm", ["--version"], { encoding: "utf8" });
if (!isAtLeast(version, minimum)) {
  console.error(
    `npm ${String(version).trim()} does not meet the Trusted Publishing requirement of npm >= ${minimum}.`,
  );
  process.exit(1);
}
console.log(`npm ${String(version).trim()} meets the Trusted Publishing requirement of npm >= ${minimum}.`);
