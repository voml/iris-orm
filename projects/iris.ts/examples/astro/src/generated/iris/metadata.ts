export const IRIS_SCHEMA_FINGERPRINT = "05f3e4048435b2053b738933be8623958eb5d9ab21d2702072e996c6216a192b";
export const IRIS_GENERATOR_VERSION = "0.1.0";
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
