import fs from "node:fs";

const config = fs.readFileSync("rust-toolchain.toml", "utf8");
const versions = [...config.matchAll(/^channel\s*=\s*"([^"]+)"\s*$/gm)];
if (versions.length !== 1 || !/^[0-9]+\.[0-9]+\.[0-9]+$/.test(versions[0][1])) {
  throw new Error("rust-toolchain.toml must specify exactly one pinned Rust version.");
}
if (!process.env.GITHUB_OUTPUT) throw new Error("GITHUB_OUTPUT must be set.");
fs.appendFileSync(process.env.GITHUB_OUTPUT, `version=${versions[0][1]}\n`);
