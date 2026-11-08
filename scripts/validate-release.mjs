import { execFileSync } from "node:child_process";
import fs from "node:fs";

const { GITHUB_REF: ref, GITHUB_REF_NAME: name, GITHUB_SHA: sha } = process.env;
if (!ref || !name || ref !== `refs/tags/${name}` || !name.startsWith("v")) {
  throw new Error("Select a v-prefixed release tag; branch and missing refs cannot publish.");
}
const { version } = JSON.parse(fs.readFileSync("package.json", "utf8"));
if (name !== `v${version}`) throw new Error(`Release tag ${name} does not match package version ${version}.`);
const tagCommit = execFileSync("git", ["rev-parse", `refs/tags/${name}^{commit}`], { encoding: "utf8" }).trim();
const checkoutCommit = execFileSync("git", ["rev-parse", "HEAD^{commit}"], { encoding: "utf8" }).trim();
if (!sha || tagCommit !== sha || checkoutCommit !== sha) {
  throw new Error("Release tag, event commit, and checked-out commit must match exactly.");
}
execFileSync(process.execPath, ["scripts/sync-native-versions.mjs", "--check"], { stdio: "inherit" });
console.log(`Validated ${name} at ${sha}.`);
