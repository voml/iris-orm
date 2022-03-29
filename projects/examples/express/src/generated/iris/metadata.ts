export const IRIS_SCHEMA_FINGERPRINT = "6aea2db8799ad291";
export const IRIS_GENERATOR_VERSION = "0.1.0";
/** TypeScript author field name → VOS wire field name (per entity). */
export const IRIS_FIELD_WIRE_NAMES: Record<string, Readonly<Record<string, string>>> = {
    User: { userId: "user_id", userName: "user_name", active: "active" },
    Post: { postId: "post_id", author: "author", title: "title", published: "published" },
};
/** Reference field → target entity (per entity). */
export const IRIS_REFERENCE_TARGETS: Record<string, Readonly<Record<string, string>>> = {
    User: {  },
    Post: { author: "User" },
};
