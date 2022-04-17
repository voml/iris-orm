export async function register() {
    if (process.env.NEXT_RUNTIME !== "nodejs") {
        return;
    }
    const { openDatabase } = await import("@iris/node.ts");
    const db = await openDatabase({ config: process.cwd(), source: "default" });
    await db.$macros.seed_blog();
}
