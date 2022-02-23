#!/usr/bin/env node
import { distDirForTarget, DEFAULT_OUT_TARGET, readDefaultProfile } from "./profile-out-dir.mjs";
import { runVmz } from "./run-vmz.mjs";

const profile = process.env.VMZ_PROFILE || readDefaultProfile();
const target = process.env.VMZ_OUT_TARGET || DEFAULT_OUT_TARGET;
const outDir = distDirForTarget(target);

console.log(`@yydb/iris-homepage serve → ${outDir} (target=${target}, profile=${profile})`);

const run = runVmz("serve", [".", "--profile", profile, "--out-dir", outDir]);
process.exit(run.status ?? 1);
