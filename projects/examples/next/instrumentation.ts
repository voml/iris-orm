export async function register() {
    if (process.env.NEXT_RUNTIME !== "nodejs") {
        return;
    }
    const { Database } = await import("@iris/node.ts");
    const db = await Database.open({ config: process.cwd(), source: "default" });
    await db.$macros.seed_blog();
}
