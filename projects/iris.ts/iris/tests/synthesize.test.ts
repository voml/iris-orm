import assert from "node:assert/strict";
import { test } from "node:test";

import {
    compileSelectProjection,
    compileWherePredicates,
    synthesizeFindMany,
    synthesizeFindUnique,
} from "../src/codegen/synthesize-vos.ts";

test("compileWherePredicates navigates reference paths", () => {
    const parameters: Record<string, unknown> = {};
    const predicates = compileWherePredicates(
        {
            author: {
                user_name: { not: "" },
            },
            title: { contains: "hello" },
        },
        parameters,
    );

    assert.equal(predicates.length, 2);
    assert.match(predicates[0]!, /x\.author\.user_name\s*!=\s*\$/);
    assert.match(predicates[1]!, /x\.title\.contains\(\$/);
    assert.equal(Object.values(parameters).includes(""), true);
    assert.equal(Object.values(parameters).includes("hello"), true);
});

test("compileWherePredicates keeps scalar equality as use-site leaf", () => {
    const parameters: Record<string, unknown> = {};
    const predicates = compileWherePredicates({ active: true }, parameters);
    assert.deepEqual(predicates, ["x.active"]);
    assert.deepEqual(parameters, {});
});

test("compileSelectProjection nests reference dereference projections", () => {
    const projection = compileSelectProjection({
        post_id: true,
        title: true,
        author: {
            user_id: true,
            user_name: true,
        },
    });

    assert.equal(
        projection,
        "x.{ post_id, title, author: x.author.{ user_id, user_name } }",
    );
});

test("synthesizeFindMany builds filter + map + collect use-site IR", () => {
    const { source, parameters } = synthesizeFindMany("Post", {
        where: {
            author: {
                user_name: { not: "" },
            },
        },
        select: {
            post_id: true,
            title: true,
            author: {
                user_id: true,
                user_name: true,
            },
        },
        take: 20,
    });

    assert.match(source, /^Post\.filter\(x => x\.author\.user_name != \$/);
    assert.match(source, /\.map\(x => x\.\{ post_id, title, author: x\.author\.\{ user_id, user_name \} \}\)/);
    assert.match(source, /\.take\(\$take\)\.collect\(\)$/);
    assert.equal(parameters.take, 20);
    assert.equal(Object.values(parameters).includes(""), true);
});

test("synthesizeFindMany where-only does not project unused reference into result map", () => {
    const { source } = synthesizeFindMany("Post", {
        where: {
            author: {
                user_name: { not: "" },
            },
        },
        select: {
            post_id: true,
            title: true,
        },
    });

    assert.match(source, /filter\(x => x\.author\.user_name/);
    assert.match(source, /\.map\(x => x\.\{ post_id, title \}\)/);
    assert.doesNotMatch(source, /author: x\.author/);
});

test("synthesizeFindUnique shares use-site rules", () => {
    const { source, parameters } = synthesizeFindUnique("Post", {
        where: { post_id: "p1" },
        select: {
            title: true,
            author: { user_name: true },
        },
    });

    assert.match(source, /Post\.filter\(x => x\.post_id == \$/);
    assert.match(source, /\.map\(x => x\.\{ title, author: x\.author\.\{ user_name \} \}\)\.collect\(\)/);
    assert.equal(Object.values(parameters).includes("p1"), true);
});

test("synthesizeCreate emits Entity::insert with bound fields (no placeholders)", async () => {
    const { synthesizeCreate, synthesizeMacroCall, flattenRefInput } = await import(
        "../src/codegen/synthesize-vos.ts"
    );

    assert.equal(flattenRefInput({ user_id: "u1", __irisRef: "User" }), "u1");

    const created = synthesizeCreate("Post", {
        data: {
            post_id: "p1",
            title: "Hello",
            author: { user_id: "u1" },
        },
    });
    assert.match(created.source, /^Post::insert\(\{ /);
    assert.match(created.source, /post_id: \$/);
    assert.match(created.source, /author: \$/);
    assert.doesNotMatch(created.source, /\.\.\./);
    assert.equal(created.parameters.data_title, "Hello");
    assert.equal(created.parameters.data_author, "u1");

    const macro = synthesizeMacroCall("public_user", [{ user_id: "u1" }]);
    assert.equal(macro.source, "public_user($arg0)");
    assert.equal(macro.parameters.arg0, "u1");
});
