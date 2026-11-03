import { createClient } from "./node.js";

function readBoolField(row: Record<string, unknown>, field: string): boolean {
    const value = row[field];
    return value === true || value === 1;
}

const db = createClient({ dsn: "sqlite:data/app.sqlite" });

const created = await db.tables.user.create({
    data: { userId: "proj-1", userName: "Ada", active: true },
    select: { userId: true, userName: true, active: true },
});

const unique = await db.tables.user.findUnique({
    where: { userId: "proj-1" },
});

const updated = await db.tables.user.update({
    where: { userId: "proj-1" },
    data: { userName: { set: "Grace" } },
});

const afterUpdate = await db.tables.user.findMany({
    where: { userId: { eq: "proj-1" } },
});

await db.tables.user.delete({ where: { userId: "proj-1" } });

const afterDelete = await db.tables.user.findMany({
    where: { userId: { eq: "proj-1" } },
});

const createdRow = created as Record<string, unknown>;

console.log(
    JSON.stringify({
        ok: true,
        createdUserId: created.userId,
        createdUserName: created.userName,
        createdActive: readBoolField(createdRow, "active"),
        uniqueUserName: unique?.userName,
        updatedUserName: updated.userName,
        afterUpdateCount: afterUpdate.length,
        afterDeleteCount: afterDelete.length,
    }),
);
