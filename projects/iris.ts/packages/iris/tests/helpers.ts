export function srcImport(relativeFromPkgRoot: string): string {
    return new URL(relativeFromPkgRoot, new URL("../", import.meta.url)).href;
}
