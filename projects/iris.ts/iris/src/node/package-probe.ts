import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

/** Whether an optional platform npm package is installed (does not load `.node`). */
export function isOptionalPackageInstalled(packageName: string): boolean {
    try {
        require.resolve(packageName);
        return true;
    } catch {
        return false;
    }
}
