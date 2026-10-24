/** Public coarse platform keys → optional native binding packages (panduck-style). */
export const IRIS_PLATFORM_PACKAGES: Record<string, string> = {
    "win32-x64": "@yydb/iris-win32-x64",
    "linux-x64": "@yydb/iris-linux-x64",
    "linux-arm64": "@yydb/iris-linux-arm64",
    "darwin-x64": "@yydb/iris-darwin-x64",
    "darwin-arm64": "@yydb/iris-darwin-arm64",
};

export function resolvePlatformPackage(platform: NodeJS.Platform, arch: string): string | null {
    return IRIS_PLATFORM_PACKAGES[`${platform}-${arch}`] ?? null;
}
