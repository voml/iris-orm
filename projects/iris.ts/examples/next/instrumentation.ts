export async function register() {
    if (process.env.NEXT_RUNTIME !== "nodejs") {
        return;
    }
    const { createClient } = await import("@iris/node.ts");
    const db = createClient({ dsn: "file:.iris/dev.yydb" });
    await db.macros.seed_blog();
}
