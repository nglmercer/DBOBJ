import { spawnSync } from "child_process";
import { existsSync } from "fs";

/**
 * Targets to build for.
 * Note: Requires `zig` and `cargo-zigbuild` for cross-compilation.
 * For Windows target on non-Windows, `cargo-xwin` is typically used by napi-rs.
 */
const targets = [
  "x86_64-unknown-linux-gnu",   // Linux x64
  "aarch64-unknown-linux-gnu",  // Linux ARM64
  "x86_64-apple-darwin",        // macOS x64
  "aarch64-apple-darwin",       // macOS ARM64 (Apple Silicon)
  "x86_64-pc-windows-msvc",     // Windows x64
];

async function runBuild() {
  console.log("🚀 DBOBJ Multi-platform Build System");
  console.log("====================================\n");

  // Ensure ~/.cargo/bin is in PATH for the script
  const home = process.env.HOME;
  const cargoBin = `${home}/.cargo/bin`;
  process.env.PATH = `${cargoBin}:${process.env.PATH}`;

  // Verify cargo-zigbuild
  const zigbuildCheck = spawnSync("cargo", ["zigbuild", "--version"]);
  const hasZigBuild = zigbuildCheck.status === 0;
  
  if (!hasZigBuild) {
    console.warn("⚠️  cargo-zigbuild not found in PATH.");
    console.log(`🔍 Checked PATH: ${process.env.PATH}`);
    console.log("👉 Try: cargo install cargo-zigbuild\n");
  }

  for (const target of targets) {
    console.log(`\n🔨 Building target: ${target}...`);
    
    const args = [
      "napi", "build",
      "--release",
      "--target", target,
      "--cross-compile",
    ];

    // Disable mimalloc for Apple targets to avoid SDK dependency issues
    if (!target.includes("apple")) {
      args.push("--features", "mimalloc");
    }

    // On Linux, building for Windows MSVC requires cargo-xwin
    if (target.includes("windows") && process.platform !== "win32") {
       console.log("ℹ️  Building for Windows requires 'cargo-xwin'. Ensure it is installed.");
    }

    const start = performance.now();
    const result = spawnSync("npx", args, {
      stdio: "inherit",
      shell: true,
    });
    const end = performance.now();

    if (result.status === 0) {
      console.log(`✅ Success: ${target} (${((end - start) / 1000).toFixed(2)}s)`);
    } else {
      console.error(`❌ Failed: ${target}`);
    }
  }

  console.log("\n✨ Multi-platform build sequence finished.");
  console.log("Check the artifacts in the root directory.");
}

runBuild().catch(console.error);
