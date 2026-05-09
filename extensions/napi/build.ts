import { spawnSync, execSync } from "child_process";
import { existsSync, readdirSync, statSync, unlinkSync } from "fs";
import { resolve, join } from "path";

// ─── Configuration ───────────────────────────────────────────────────────────

const NAPI_DIR = import.meta.dir;
const OUTPUT_DIR = NAPI_DIR; // .node files land next to package.json
const PACKAGE_NAME = "dbobj";

interface Target {
  triple: string;
  label: string;
  /** Features to pass (omit mimalloc on macOS to avoid SDK issues) */
  features?: string[];
  /** Required cross-compile tool: "zigbuild" | "xwin" | null (native) */
  crossTool: "zigbuild" | "xwin" | null;
}

const ALL_TARGETS: Target[] = [
  { triple: "x86_64-unknown-linux-gnu",  label: "Linux x64",        features: ["mimalloc"], crossTool: null },
  { triple: "aarch64-unknown-linux-gnu", label: "Linux ARM64",      features: ["mimalloc"], crossTool: "zigbuild" },
  { triple: "x86_64-apple-darwin",       label: "macOS x64",        crossTool: "zigbuild" },
  { triple: "aarch64-apple-darwin",      label: "macOS ARM64",      crossTool: "zigbuild" },
  { triple: "x86_64-pc-windows-msvc",    label: "Windows x64",      crossTool: "xwin" },
  { triple: "aarch64-pc-windows-msvc",   label: "Windows ARM64",    crossTool: "xwin" },
];

// ─── Helpers ─────────────────────────────────────────────────────────────────

/** Detect the host triple via `rustc -vV` */
function getHostTriple(): string {
  try {
    const out = execSync("rustc -vV", { encoding: "utf-8" });
    const match = out.match(/host:\s*(\S+)/);
    return match?.[1] ?? "x86_64-unknown-linux-gnu";
  } catch {
    return "x86_64-unknown-linux-gnu";
  }
}

function commandExists(cmd: string, args: string[] = ["--version"]): boolean {
  const r = spawnSync(cmd, args, { stdio: "ignore", shell: true });
  return r.status === 0;
}

function hasRustTarget(triple: string): boolean {
  const r = spawnSync("rustup", ["target", "list", "--installed"], {
    encoding: "utf-8",
    stdio: ["pipe", "pipe", "pipe"],
  });
  return r.status === 0 && (r.stdout ?? "").includes(triple);
}

function installRustTarget(triple: string): boolean {
  console.log(`  📦 Installing Rust target: ${triple}`);
  const r = spawnSync("rustup", ["target", "add", triple], { stdio: "inherit" });
  return r.status === 0;
}

function formatTime(ms: number): string {
  return (ms / 1000).toFixed(2) + "s";
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
  return (bytes / (1024 * 1024)).toFixed(1) + " MB";
}

// ─── Build a single target ──────────────────────────────────────────────────

interface BuildResult {
  target: Target;
  success: boolean;
  skipped: boolean;
  elapsed: number;
  outputFile?: string;
  error?: string;
}

function buildTarget(target: Target, hostTriple: string): BuildResult {
  const isNative = target.triple === hostTriple;
  const start = performance.now();

  // 1. Check cross-compilation prerequisites
  if (!isNative) {
    if (target.crossTool === "zigbuild" && !commandExists("cargo-zigbuild", ["--version"])) {
      return {
        target, success: false, skipped: true,
        elapsed: performance.now() - start,
        error: "cargo-zigbuild not installed (cargo install cargo-zigbuild)",
      };
    }
    if (target.crossTool === "xwin" && !commandExists("cargo-xwin", ["--version"])) {
      return {
        target, success: false, skipped: true,
        elapsed: performance.now() - start,
        error: "cargo-xwin not installed (cargo install cargo-xwin)",
      };
    }
  }

  // 2. Ensure the Rust target is installed
  if (!hasRustTarget(target.triple)) {
    if (!installRustTarget(target.triple)) {
      return {
        target, success: false, skipped: true,
        elapsed: performance.now() - start,
        error: `Failed to install Rust target ${target.triple}`,
      };
    }
  }

  // 3. Build the napi build command
  const args: string[] = [
    "napi", "build",
    "--release",
    "--platform",
    "--target", target.triple,
    "--output-dir", OUTPUT_DIR,
  ];

  // Only use --cross-compile for non-native targets
  if (!isNative) {
    args.push("--cross-compile");
  }

  // Add features
  if (target.features && target.features.length > 0) {
    args.push("--features", target.features.join(","));
  }

  // 4. Run the build
  const result = spawnSync("npx", args, {
    cwd: NAPI_DIR,
    stdio: "inherit",
    shell: true,
    env: {
      ...process.env,
      PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}`,
    },
  });

  const elapsed = performance.now() - start;

  if (result.status !== 0) {
    return {
      target, success: false, skipped: false, elapsed,
      error: `Build exited with code ${result.status}`,
    };
  }

  // 5. Verify the .node file was created
  const nodeFile = findNodeFileForTarget(target.triple);
  if (!nodeFile) {
    return {
      target, success: false, skipped: false, elapsed,
      error: "Build succeeded but no .node file was produced",
    };
  }

  return {
    target, success: true, skipped: false, elapsed,
    outputFile: nodeFile,
  };
}

/**
 * Map a Rust target triple to the napi-rs platform suffix.
 * e.g. x86_64-unknown-linux-gnu → linux-x64-gnu
 */
function tripleToPlatformSuffix(triple: string): string {
  const map: Record<string, string> = {
    "x86_64-unknown-linux-gnu":  "linux-x64-gnu",
    "aarch64-unknown-linux-gnu": "linux-arm64-gnu",
    "x86_64-apple-darwin":       "darwin-x64",
    "aarch64-apple-darwin":      "darwin-arm64",
    "x86_64-pc-windows-msvc":    "win32-x64-msvc",
    "aarch64-pc-windows-msvc":   "win32-arm64-msvc",
  };
  return map[triple] ?? triple;
}

/** Find the .node file for a specific target */
function findNodeFileForTarget(triple: string): string | null {
  const suffix = tripleToPlatformSuffix(triple);
  try {
    const match = readdirSync(OUTPUT_DIR).find(
      (f) => f.endsWith(".node") && f.includes(suffix)
    );
    return match ? join(OUTPUT_DIR, match) : null;
  } catch {
    return null;
  }
}

// ─── CLI ─────────────────────────────────────────────────────────────────────

type Mode = "native" | "all" | "target";

function parseArgs(): { mode: Mode; specificTarget?: string } {
  const args = process.argv.slice(2);

  if (args.includes("--native") || args.includes("-n")) {
    return { mode: "native" };
  }

  const targetIdx = args.indexOf("--target");
  if (targetIdx !== -1 && args[targetIdx + 1]) {
    return { mode: "target", specificTarget: args[targetIdx + 1] };
  }

  const tIdx = args.indexOf("-t");
  if (tIdx !== -1 && args[tIdx + 1]) {
    return { mode: "target", specificTarget: args[tIdx + 1] };
  }

  return { mode: "all" };
}

// ─── Main ────────────────────────────────────────────────────────────────────

async function main() {
  console.log("");
  console.log("  ╔══════════════════════════════════════════╗");
  console.log("  ║     🚀 DBOBJ NAPI Build System           ║");
  console.log("  ╚══════════════════════════════════════════╝");
  console.log("");

  const hostTriple = getHostTriple();
  const { mode, specificTarget } = parseArgs();

  console.log(`  Host:    ${hostTriple}`);
  console.log(`  Mode:    ${mode}`);
  console.log(`  Output:  ${OUTPUT_DIR}`);
  console.log("");

  // Determine which targets to build
  let targets: Target[];

  if (mode === "native") {
    const native = ALL_TARGETS.find((t) => t.triple === hostTriple);
    targets = native ? [native] : [{ triple: hostTriple, label: "Native", crossTool: null }];
  } else if (mode === "target" && specificTarget) {
    const found = ALL_TARGETS.find((t) => t.triple === specificTarget);
    if (!found) {
      console.error(`  ❌ Unknown target: ${specificTarget}`);
      console.log(`  Available: ${ALL_TARGETS.map((t) => t.triple).join(", ")}`);
      process.exit(1);
    }
    targets = [found];
  } else {
    targets = ALL_TARGETS;
  }

  // Pre-flight: check tooling
  const hasZigbuild = commandExists("cargo-zigbuild", ["--version"]);
  const hasXwin = commandExists("cargo-xwin", ["--version"]);

  if (mode === "all") {
    console.log("  Tooling:");
    console.log(`    cargo-zigbuild: ${hasZigbuild ? "✅" : "❌ (needed for cross-compile to macOS/Linux-ARM)"}`);
    console.log(`    cargo-xwin:     ${hasXwin ? "✅" : "❌ (needed for cross-compile to Windows)"}`);
    console.log("");
  }

  // Build each target
  const results: BuildResult[] = [];
  const separator = "  " + "─".repeat(50);

  for (const target of targets) {
    console.log(separator);
    const isNative = target.triple === hostTriple;
    console.log(`  🔨 ${target.label} (${target.triple})${isNative ? " [native]" : ""}`);
    console.log("");

    const result = buildTarget(target, hostTriple);
    results.push(result);

    if (result.skipped) {
      console.log(`  ⏭️  Skipped: ${result.error}`);
    } else if (result.success) {
      const file = result.outputFile!;
      const size = formatSize(statSync(file).size);
      console.log(`  ✅ Built in ${formatTime(result.elapsed)} → ${file.replace(NAPI_DIR + "/", "")} (${size})`);
    } else {
      console.log(`  ❌ Failed (${formatTime(result.elapsed)}): ${result.error}`);
    }
    console.log("");
  }

  // Summary
  console.log(separator);
  console.log("");
  const built = results.filter((r) => r.success);
  const skipped = results.filter((r) => r.skipped);
  const failed = results.filter((r) => !r.success && !r.skipped);

  console.log("  📊 Summary:");
  console.log(`    ✅ Built:   ${built.length}`);
  if (skipped.length > 0) console.log(`    ⏭️  Skipped: ${skipped.length}`);
  if (failed.length > 0) console.log(`    ❌ Failed:  ${failed.length}`);
  console.log("");

  // List all .node files
  const nodeFiles = readdirSync(OUTPUT_DIR).filter((f) => f.endsWith(".node"));
  if (nodeFiles.length > 0) {
    console.log("  📦 Output files:");
    for (const f of nodeFiles) {
      const fullPath = join(OUTPUT_DIR, f);
      const size = formatSize(statSync(fullPath).size);
      console.log(`    • ${f} (${size})`);
    }
  } else {
    console.log("  ⚠️  No .node files found in output directory.");
  }

  console.log("");

  if (failed.length > 0) {
    process.exit(1);
  }
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
