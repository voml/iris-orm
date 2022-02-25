export const siteUrl = "https://iris-orm.pages.dev/";
export const githubUrl = "https://github.com/voml/iris-orm";

export function docsHomePath(localeId = "zh-hans"): string {
    return localeId === "en-us" ? "/d/en-us/" : "/d/zh-hans/";
}
