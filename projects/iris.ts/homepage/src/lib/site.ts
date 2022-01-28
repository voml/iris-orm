export const githubUrl = "https://github.com/yy-database/iris-orm";

export function docsHomePath(localeId = "zh-hans"): string {
    return localeId === "en-us" ? "/d/en-us/" : "/d/zh-hans/";
}
