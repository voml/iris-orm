import type { NextConfig } from "next";

const irisNativePackages = [
    "@yydb/iris",
    "@yydb/iris-win32-x64",
    "@yydb/iris-linux-x64",
    "@yydb/iris-linux-arm64",
    "@yydb/iris-darwin-x64",
    "@yydb/iris-darwin-arm64",
];

const nextConfig: NextConfig = {
    serverExternalPackages: irisNativePackages,
};

export default nextConfig;
