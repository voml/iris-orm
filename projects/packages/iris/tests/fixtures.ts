export const USER_SCHEMA = `
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
`;

export const USER_WIRE_NAMES = {
    User: { userId: "user_id", userName: "user_name", active: "active" },
} as const;

/** Matches Rust iris-wasm/tests/check_source.rs and N-API smoke fixtures. */
export const USER_SCHEMA_FINGERPRINT = "a7ddf821fff48050";

export const INVALID_SCHEMA = "not a schema";
