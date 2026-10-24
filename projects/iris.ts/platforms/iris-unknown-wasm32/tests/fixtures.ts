export const USER_SCHEMA = `
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
`;

export const USER_SCHEMA_FINGERPRINT = "60adfe744c724499";

export const INVALID_SCHEMA = "not a schema";
