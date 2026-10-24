export function srcImport(relativeFromPkgRoot: string): string {
    return new URL(relativeFromPkgRoot, new URL("../", import.meta.url)).href;
}

/**
 * Gate a test on the native build's advertised SQLite capability.
 *
 * The writable SQLite provider is currently unavailable upstream, so release
 * native builds ship without the `sqlite` feature. SQLite-dependent tests must
 * skip instead of asserting on a capability that is not part of the build.
 */
export function requireNativeSqlite(
    t: { skip: (message?: string) => void },
    bindings: { sqliteAdapterEnabled?: () => boolean } | null | undefined,
): boolean {
    if (bindings?.sqliteAdapterEnabled?.() === true) {
        return true;
    }
    t.skip("native build lacks the `sqlite` capability (iris-napi `sqlite` feature is off)");
    return false;
}

/** Load the native core and gate the test on its advertised SQLite capability. */
export function requireNativeSqliteFrom(
    t: { skip: (message?: string) => void },
    load: () => { sqliteAdapterEnabled?: () => boolean },
): boolean {
    let bindings;
    try {
        bindings = load();
    } catch (error) {
        if (error instanceof Error && "code" in error && error.code === "native-package-missing") {
            t.skip("Node semantic core not installed");
            return false;
        }
        throw error;
    }
    return requireNativeSqlite(t, bindings);
}
