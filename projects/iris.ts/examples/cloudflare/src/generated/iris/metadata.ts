export const IRIS_SCHEMA_FINGERPRINT = "05f3e4048435b2053b738933be8623958eb5d9ab21d2702072e996c6216a192b";
export const IRIS_GENERATOR_VERSION = "0.1.0";
/** VOS schema text this client was generated from (applied by managed adapters). */
export const IRIS_SCHEMA_SOURCE: string = "table User {\r\n    @@user_id: uuid,\r\n    user_name: utf8,\r\n    active: bool,\r\n}\r\n\r\ntable Post {\r\n    @@post_id: uuid,\r\n    author: &User,\r\n    title: utf8,\r\n    published: bool,\r\n}\r\n\n\nmacro seed_blog() -> unit {\r\n    User {\r\n        user_id: \"550e8400-e29b-41d4-a716-446655440000\",\r\n        user_name: \"ada\",\r\n        active: true,\r\n    }.insert()\r\n    User {\r\n        user_id: \"6ba7b810-9dad-11d1-80b4-00c04fd430c8\",\r\n        user_name: \"linus\",\r\n        active: true,\r\n    }.insert()\r\n    Post {\r\n        post_id: \"11111111-1111-4111-8111-111111111101\",\r\n        author: \"550e8400-e29b-41d4-a716-446655440000\",\r\n        title: \"Ada post\",\r\n        published: true,\r\n    }.insert()\r\n    Post {\r\n        post_id: \"22222222-2222-4222-8222-222222222202\",\r\n        author: \"6ba7b810-9dad-11d1-80b4-00c04fd430c8\",\r\n        title: \"Linus post\",\r\n        published: true,\r\n    }.insert()\r\n}\r\n";
/** TypeScript author field name → VOS wire field name (per entity). */
export const IRIS_FIELD_WIRE_NAMES: Record<string, Readonly<Record<string, string>>> = {
    User: { userId: "user_id", userName: "user_name", active: "active" },
    Post: { postId: "post_id", author: "author", title: "title", published: "published" },
};
/** TypeScript author field name → VOS value carrier label (per entity). */
export const IRIS_VALUE_CARRIERS: Record<string, Readonly<Record<string, string>>> = {
    User: { userId: "uuid", userName: "utf8", active: "bool" },
    Post: { postId: "uuid", author: "User", title: "utf8", published: "bool" },
};
/** Reference field → target entity (per entity). */
export const IRIS_REFERENCE_TARGETS: Record<string, Readonly<Record<string, string>>> = {
    User: {  },
    Post: { author: "User" },
};
