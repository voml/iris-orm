import { Database } from "./node.js";

function readBoolField(row: Record<string, unknown>, field: string): boolean {
    const value = row[field];
    return value === true || value === 1;
}

const db = await Database.open();

const created = await db.user.create({
    data: { userId: "proj-1", userName: "Ada", active: true },
    select: { userId: true, userName: true, active: true },
});

const unique = await db.user.findUnique({
    where: { userId: "proj-1" },
});

const updated = await db.user.update({
    where: { userId: "proj-1" },
    data: { userName: { set: "Grace" } },
});

const afterUpdate = await db.user.findMany({
    where: { userId: { eq: "proj-1" } },
});

await db.user.delete({ where: { userId: "proj-1" } });

const afterDelete = await db.user.findMany({
    where: { userId: { eq: "proj-1" } },
});

await Database.close();

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
