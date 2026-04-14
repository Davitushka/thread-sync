import { existsSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";

const staticRoot = resolve(process.cwd(), "../static");

function removeTarget(targetPath) {
  rmSync(targetPath, { recursive: true, force: true });
  if (!existsSync(targetPath)) {
    return;
  }

  if (process.platform === "win32") {
    const escaped = targetPath.replace(/'/g, "''");
    execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-Command",
        `Remove-Item -LiteralPath '${escaped}' -Recurse -Force -ErrorAction Stop`,
      ],
      { stdio: "ignore" }
    );
    return;
  }

  throw new Error(`Failed to clean generated target: ${targetPath}`);
}

removeTarget(resolve(staticRoot, "assets"));
removeTarget(resolve(staticRoot, "index.html"));
removeTarget(resolve(staticRoot, "app.js"));
removeTarget(resolve(staticRoot, "app.css"));
